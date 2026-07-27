use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAvailability {
    pub available: bool,
    pub current_version: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub error_kind: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgressEvent {
    pub downloaded: u64,
    pub total: Option<u64>,
    pub percent: Option<u32>,
}

static UPDATE_BUSY: AtomicBool = AtomicBool::new(false);

struct UpdateGuard;

impl UpdateGuard {
    fn try_acquire() -> Option<Self> {
        if UPDATE_BUSY
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            Some(Self)
        } else {
            None
        }
    }
}

impl Drop for UpdateGuard {
    fn drop(&mut self) {
        UPDATE_BUSY.store(false, Ordering::SeqCst);
    }
}

pub fn classify_update_error(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("signature")
        || lower.contains("minisign")
        || lower.contains("verif")
        || lower.contains("pubkey")
        || lower.contains("public key")
    {
        "signature"
    } else if lower.contains("space")
        || lower.contains("disk")
        || lower.contains("no space")
        || lower.contains("enospc")
    {
        "disk"
    } else if lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("connection")
        || lower.contains("network")
        || lower.contains("dns")
        || lower.contains("offline")
        || lower.contains("failed to fetch")
        || lower.contains("error sending request")
    {
        "network"
    } else if lower.contains("install") || lower.contains("extract") || lower.contains("replace") {
        "install"
    } else {
        "unknown"
    }
}

/// Accumulate chunk lengths into a running total and derive progress for the UI.
pub fn accumulate_download_progress(
    downloaded_total: u64,
    chunk_length: u64,
    content_length: Option<u64>,
) -> (u64, UpdateProgressEvent) {
    let downloaded = downloaded_total.saturating_add(chunk_length);
    let percent = match content_length {
        None => None,
        Some(0) => Some(0),
        Some(total) => {
            let capped = downloaded.min(total);
            Some(((capped as f64 / total as f64) * 100.0).round() as u32)
        }
    };
    (
        downloaded,
        UpdateProgressEvent {
            downloaded,
            total: content_length,
            percent,
        },
    )
}

pub fn download_complete_progress(
    downloaded: u64,
    content_length: Option<u64>,
) -> UpdateProgressEvent {
    UpdateProgressEvent {
        downloaded,
        total: content_length.or(Some(downloaded)),
        percent: Some(100),
    }
}

#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> Result<UpdateAvailability, String> {
    let current_version = app.package_info().version.to_string();
    let Some(_guard) = UpdateGuard::try_acquire() else {
        return Err("update_busy".into());
    };
    let updater = app
        .updater_builder()
        .build()
        .map_err(|error| error.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => Ok(UpdateAvailability {
            available: true,
            current_version,
            version: Some(update.version.clone()),
            notes: update.body.clone(),
            error_kind: None,
        }),
        Ok(None) => Ok(UpdateAvailability {
            available: false,
            current_version,
            version: None,
            notes: None,
            error_kind: None,
        }),
        Err(error) => {
            let message = error.to_string();
            Ok(UpdateAvailability {
                available: false,
                current_version,
                version: None,
                notes: None,
                error_kind: Some(classify_update_error(&message).into()),
            })
        }
    }
}

#[tauri::command]
pub async fn install_app_update(app: AppHandle) -> Result<(), String> {
    let Some(_guard) = UpdateGuard::try_acquire() else {
        return Err("update_busy".into());
    };
    let updater = app.updater_builder().build().map_err(|error| {
        let message = error.to_string();
        format!("{}:{}", classify_update_error(&message), message)
    })?;
    let Some(update) = updater.check().await.map_err(|error| {
        let message = error.to_string();
        format!("{}:{}", classify_update_error(&message), message)
    })?
    else {
        return Err("no_update:No update available".into());
    };

    let emit_app = app.clone();
    let downloaded_total = Arc::new(AtomicU64::new(0));
    let last_total = Arc::new(AtomicU64::new(0));
    let progress_counter = downloaded_total.clone();
    let total_counter = last_total.clone();
    let progress_app = emit_app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                let chunk_length = u64::try_from(chunk).unwrap_or(0);
                let content_length = total;
                if let Some(value) = content_length {
                    total_counter.store(value, Ordering::SeqCst);
                }
                let previous = progress_counter.load(Ordering::SeqCst);
                let (downloaded, event) =
                    accumulate_download_progress(previous, chunk_length, content_length);
                progress_counter.store(downloaded, Ordering::SeqCst);
                let _ = progress_app.emit("updater:progress", event);
            },
            move || {
                let downloaded = downloaded_total.load(Ordering::SeqCst);
                let stored_total = last_total.load(Ordering::SeqCst);
                let content_length = if stored_total > 0 {
                    Some(stored_total)
                } else {
                    None
                };
                let _ = emit_app.emit(
                    "updater:progress",
                    download_complete_progress(downloaded, content_length),
                );
            },
        )
        .await
        .map_err(|error| {
            let message = error.to_string();
            format!("{}:{}", classify_update_error(&message), message)
        })?;
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::{
        accumulate_download_progress, classify_update_error, download_complete_progress,
        UpdateProgressEvent,
    };

    #[test]
    fn classifies_update_errors() {
        assert_eq!(classify_update_error("signature mismatch"), "signature");
        assert_eq!(classify_update_error("error sending request"), "network");
        assert_eq!(classify_update_error("No space left on device"), "disk");
        assert_eq!(classify_update_error("failed to install update"), "install");
        assert_eq!(classify_update_error("weird failure"), "unknown");
    }

    #[test]
    fn accumulates_multiple_chunks_against_known_total() {
        let (total, first) = accumulate_download_progress(0, 25, Some(100));
        assert_eq!(total, 25);
        assert_eq!(first.percent, Some(25));
        let (total, second) = accumulate_download_progress(total, 25, Some(100));
        assert_eq!(total, 50);
        assert_eq!(second.percent, Some(50));
        let (total, third) = accumulate_download_progress(total, 60, Some(100));
        assert_eq!(total, 110);
        assert_eq!(
            third,
            UpdateProgressEvent {
                downloaded: 110,
                total: Some(100),
                percent: Some(100)
            }
        );
    }

    #[test]
    fn unknown_or_zero_total_keeps_percent_rules() {
        let (_, unknown) = accumulate_download_progress(10, 5, None);
        assert_eq!(unknown.percent, None);
        assert_eq!(unknown.downloaded, 15);
        let (_, zero) = accumulate_download_progress(0, 8, Some(0));
        assert_eq!(zero.percent, Some(0));
    }

    #[test]
    fn download_complete_emits_full_percent() {
        let done = download_complete_progress(42, None);
        assert_eq!(done.percent, Some(100));
        assert_eq!(done.total, Some(42));
        let done = download_complete_progress(80, Some(100));
        assert_eq!(done.percent, Some(100));
        assert_eq!(done.total, Some(100));
    }
}
