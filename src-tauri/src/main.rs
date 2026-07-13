use base64::{
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use chrono::Utc;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewWindow,
};
use tauri_plugin_autostart::ManagerExt;

const DEFAULT_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const FIVE_HOURS_SECONDS: i64 = 18_000;
const SEVEN_DAYS_SECONDS: i64 = 604_800;

#[derive(Clone, Debug, Default, Deserialize)]
struct CodexAuthFile {
    auth_mode: Option<String>,
    tokens: Option<CodexTokens>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct CodexTokens {
    access_token: Option<String>,
    id_token: Option<String>,
    account_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct UsageResponse {
    plan_type: Option<String>,
    rate_limit: Option<RateLimitDetails>,
}

#[derive(Clone, Debug, Deserialize)]
struct RateLimitDetails {
    primary_window: Option<UsageWindowResponse>,
    secondary_window: Option<UsageWindowResponse>,
}

#[derive(Clone, Debug, Deserialize)]
struct UsageWindowResponse {
    used_percent: f64,
    limit_window_seconds: i64,
    reset_after_seconds: Option<i64>,
    reset_at: i64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuotaWindows {
    five_hour: Option<QuotaWindow>,
    seven_day: Option<QuotaWindow>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuotaWindow {
    used_percent: f64,
    remaining_percent: f64,
    duration_seconds: i64,
    reset_after_seconds: Option<i64>,
    resets_at: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountSummary {
    display_name: String,
    plan: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderStatus {
    connected: bool,
    state: String,
    message: String,
    source: String,
    auth_path_label: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MonitorSnapshot {
    account: AccountSummary,
    provider: ProviderStatus,
    windows: QuotaWindows,
    refreshed_at: Option<String>,
    checked_at: String,
    cached: bool,
}

#[derive(Default)]
struct MonitorState {
    snapshot: Mutex<Option<MonitorSnapshot>>,
}

#[derive(Debug)]
struct ProviderFailure {
    state: &'static str,
    message: String,
}

fn provider_failure(state: &'static str, message: impl Into<String>) -> ProviderFailure {
    ProviderFailure {
        state,
        message: message.into(),
    }
}

fn codex_home() -> Result<PathBuf, ProviderFailure> {
    if let Some(path) = env::var_os("CODEX_HOME") {
        return Ok(PathBuf::from(path));
    }

    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or_else(|| {
            provider_failure(
                "auth_missing",
                "Could not locate the current user's home directory.",
            )
        })?;
    Ok(PathBuf::from(home).join(".codex"))
}

fn auth_path_label(_home: &Path) -> String {
    if env::var_os("CODEX_HOME").is_some() {
        "$CODEX_HOME/auth.json".into()
    } else if cfg!(target_os = "windows") {
        "%USERPROFILE%\\.codex\\auth.json".into()
    } else {
        "~/.codex/auth.json".into()
    }
}

fn read_auth(home: &Path) -> Result<CodexAuthFile, ProviderFailure> {
    let path = home.join("auth.json");
    let contents = fs::read_to_string(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            provider_failure(
                "auth_missing",
                "Codex is not signed in on this computer. Run `codex login` first.",
            )
        } else {
            provider_failure(
                "auth_unreadable",
                "The local Codex sign-in file could not be read.",
            )
        }
    })?;

    serde_json::from_str(&contents).map_err(|_| {
        provider_failure(
            "auth_invalid",
            "The local Codex sign-in file is not valid JSON.",
        )
    })
}

fn decode_jwt_claims(token: Option<&str>) -> Option<Value> {
    let payload = token?.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| URL_SAFE.decode(payload))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn claim_string(claims: &Value, key: &str) -> Option<String> {
    claims.get(key)?.as_str().map(str::to_owned)
}

fn account_identity(tokens: &CodexTokens, response_plan: Option<&str>) -> AccountSummary {
    let claims = decode_jwt_claims(tokens.id_token.as_deref()).unwrap_or(Value::Null);
    let auth_claims = claims
        .get("https://api.openai.com/auth")
        .unwrap_or(&Value::Null);
    let display_name = claim_string(&claims, "email")
        .or_else(|| claim_string(&claims, "name"))
        .unwrap_or_else(|| "Local Codex account".into());
    let plan = response_plan
        .map(str::to_owned)
        .or_else(|| claim_string(auth_claims, "chatgpt_plan_type"))
        .unwrap_or_else(|| "ChatGPT".into());
    AccountSummary { display_name, plan }
}

fn configured_usage_url(home: &Path) -> String {
    let config = match fs::read_to_string(home.join("config.toml")) {
        Ok(value) => value,
        Err(_) => return DEFAULT_USAGE_URL.into(),
    };

    for raw_line in config.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "chatgpt_base_url" {
            continue;
        }
        let base = value.trim().trim_matches(['\'', '"']).trim_end_matches('/');
        if base.starts_with("https://chatgpt.com") || base.starts_with("https://chat.openai.com") {
            let normalized = if base.contains("/backend-api") {
                base.to_string()
            } else {
                format!("{base}/backend-api")
            };
            return format!("{normalized}/wham/usage");
        }
    }

    DEFAULT_USAGE_URL.into()
}

fn quota_window(window: UsageWindowResponse) -> Option<QuotaWindow> {
    let resets_at = chrono::DateTime::from_timestamp(window.reset_at, 0)?.to_rfc3339();
    let used = window.used_percent.clamp(0.0, 100.0);
    Some(QuotaWindow {
        used_percent: used,
        remaining_percent: 100.0 - used,
        duration_seconds: window.limit_window_seconds,
        reset_after_seconds: window.reset_after_seconds,
        resets_at,
    })
}

fn classify_windows(rate_limit: Option<RateLimitDetails>) -> QuotaWindows {
    let mut result = QuotaWindows::default();
    let Some(rate_limit) = rate_limit else {
        return result;
    };

    for window in [rate_limit.primary_window, rate_limit.secondary_window]
        .into_iter()
        .flatten()
    {
        let duration = window.limit_window_seconds;
        let mapped = quota_window(window);
        if mapped.is_none() {
            continue;
        }
        let distance_five =
            (duration - FIVE_HOURS_SECONDS).abs() as f64 / FIVE_HOURS_SECONDS as f64;
        let distance_week =
            (duration - SEVEN_DAYS_SECONDS).abs() as f64 / SEVEN_DAYS_SECONDS as f64;
        if distance_five <= 0.10 && result.five_hour.is_none() {
            result.five_hour = mapped;
        } else if distance_week <= 0.10 && result.seven_day.is_none() {
            result.seven_day = mapped;
        }
    }
    result
}

async fn fetch_local_snapshot() -> Result<MonitorSnapshot, ProviderFailure> {
    let home = codex_home()?;
    let label = auth_path_label(&home);
    let auth = read_auth(&home)?;
    if auth
        .auth_mode
        .as_deref()
        .is_some_and(|mode| !mode.eq_ignore_ascii_case("chatgpt"))
    {
        return Err(provider_failure("unsupported_auth", "The current Codex session uses API-key mode. Sign in with ChatGPT to read subscription limits."));
    }
    let tokens = auth.tokens.ok_or_else(|| {
        provider_failure(
            "auth_missing",
            "No ChatGPT OAuth session was found. Run `codex login`.",
        )
    })?;
    let access_token = tokens
        .access_token
        .as_deref()
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| {
            provider_failure(
                "auth_missing",
                "The local Codex session does not contain an access token. Run `codex login`.",
            )
        })?;

    let client = Client::builder()
        .no_gzip()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|_| {
            provider_failure(
                "network_error",
                "Could not initialize the secure usage connection.",
            )
        })?;
    let mut request = client
        .get(configured_usage_url(&home))
        .bearer_auth(access_token)
        .header("Accept", "application/json")
        .header("Cache-Control", "no-cache, no-store, max-age=0")
        .header("Pragma", "no-cache")
        .header("User-Agent", "codex-cli");
    if let Some(account_id) = tokens
        .account_id
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        request = request.header("ChatGPT-Account-Id", account_id);
    }

    let response = request.send().await.map_err(|_| {
        provider_failure(
            "network_error",
            "The Codex usage service could not be reached. Check the network and try again.",
        )
    })?;
    if matches!(
        response.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        return Err(provider_failure(
            "reauth_required",
            "The local Codex session has expired or is not authorized. Run `codex login` again.",
        ));
    }
    if !response.status().is_success() {
        return Err(provider_failure(
            "service_error",
            format!(
                "The Codex usage service returned HTTP {}.",
                response.status().as_u16()
            ),
        ));
    }
    let payload: UsageResponse = response.json().await.map_err(|_| {
        provider_failure(
            "invalid_response",
            "The Codex usage response could not be understood.",
        )
    })?;
    let account = account_identity(&tokens, payload.plan_type.as_deref());
    let windows = classify_windows(payload.rate_limit);
    let state = if windows.five_hour.is_some() && windows.seven_day.is_some() {
        "connected"
    } else {
        "partial"
    };
    let message = if state == "connected" {
        "Live limits from the local Codex sign-in session.".into()
    } else {
        "Connected. This account currently exposes only some quota windows.".into()
    };
    let now = Utc::now().to_rfc3339();
    Ok(MonitorSnapshot {
        account,
        provider: ProviderStatus {
            connected: true,
            state: state.into(),
            message,
            source: "local_codex_oauth".into(),
            auth_path_label: label,
        },
        windows,
        refreshed_at: Some(now.clone()),
        checked_at: now,
        cached: false,
    })
}

fn failure_snapshot(failure: ProviderFailure) -> MonitorSnapshot {
    let label = codex_home()
        .ok()
        .as_deref()
        .map(auth_path_label)
        .unwrap_or_else(|| "Codex auth.json".into());
    MonitorSnapshot {
        account: AccountSummary {
            display_name: "Local Codex account".into(),
            plan: "—".into(),
        },
        provider: ProviderStatus {
            connected: false,
            state: failure.state.into(),
            message: failure.message,
            source: "local_codex_oauth".into(),
            auth_path_label: label,
        },
        windows: QuotaWindows::default(),
        refreshed_at: None,
        checked_at: Utc::now().to_rfc3339(),
        cached: false,
    }
}

fn update_tray_tooltip(app: &AppHandle, snapshot: &MonitorSnapshot) {
    let five = snapshot
        .windows
        .five_hour
        .as_ref()
        .map(|window| format!("5h {:.0}%", window.remaining_percent))
        .unwrap_or_else(|| "5h —".into());
    let week = snapshot
        .windows
        .seven_day
        .as_ref()
        .map(|window| format!("7d {:.0}%", window.remaining_percent))
        .unwrap_or_else(|| "7d —".into());
    if let Some(tray) = app.tray_by_id("monitor-tray") {
        let _ = tray.set_tooltip(Some(format!("Codex Usage Monitor · {five} · {week}")));
    }
}

#[tauri::command]
async fn refresh_monitor_data(
    app: AppHandle,
    state: tauri::State<'_, MonitorState>,
) -> Result<MonitorSnapshot, String> {
    let snapshot = match fetch_local_snapshot().await {
        Ok(snapshot) => snapshot,
        Err(failure) => {
            let cached = state
                .snapshot
                .lock()
                .map_err(|_| "Snapshot cache is unavailable".to_string())?
                .clone();
            if let Some(mut cached) = cached {
                cached.provider.connected = false;
                cached.provider.state = "stale".into();
                cached.provider.message =
                    format!("{} Showing the last successful snapshot.", failure.message);
                cached.checked_at = Utc::now().to_rfc3339();
                cached.cached = true;
                cached
            } else {
                failure_snapshot(failure)
            }
        }
    };
    if !snapshot.cached && snapshot.provider.connected {
        *state
            .snapshot
            .lock()
            .map_err(|_| "Snapshot cache is unavailable".to_string())? = Some(snapshot.clone());
    }
    update_tray_tooltip(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
async fn get_monitor_snapshot(
    app: AppHandle,
    state: tauri::State<'_, MonitorState>,
) -> Result<MonitorSnapshot, String> {
    refresh_monitor_data(app, state).await
}

#[tauri::command]
fn start_window_drag(window: WebviewWindow) -> Result<(), String> {
    window.start_dragging().map_err(|error| error.to_string())
}

#[tauri::command]
fn set_always_on_top(window: WebviewWindow, enabled: bool) -> Result<(), String> {
    window
        .set_always_on_top(enabled)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_start_at_login(app: AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|error| error.to_string())
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Open Codex Usage Monitor", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh usage", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &refresh, &quit])?;
    TrayIconBuilder::with_id("monitor-tray")
        .tooltip("Codex Usage Monitor")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "refresh" => {
                let _ = app.emit("monitor:refresh", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(event, tauri::tray::TrayIconEvent::DoubleClick { .. }) {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .manage(MonitorState::default())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            setup_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_monitor_snapshot,
            refresh_monitor_data,
            start_window_drag,
            set_always_on_top,
            set_start_at_login
        ])
        .run(tauri::generate_context!())
        .expect("error while running Codex Usage Monitor");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(used: f64, duration: i64, reset_at: i64) -> UsageWindowResponse {
        UsageWindowResponse {
            used_percent: used,
            limit_window_seconds: duration,
            reset_after_seconds: Some(10),
            reset_at,
        }
    }

    #[test]
    fn maps_windows_by_duration_not_api_position() {
        let rate_limit = RateLimitDetails {
            primary_window: Some(window(41.0, SEVEN_DAYS_SECONDS, 1_800_000_000)),
            secondary_window: Some(window(25.0, FIVE_HOURS_SECONDS, 1_800_000_100)),
        };
        let mapped = classify_windows(Some(rate_limit));
        assert_eq!(mapped.five_hour.unwrap().remaining_percent, 75.0);
        assert_eq!(mapped.seven_day.unwrap().remaining_percent, 59.0);
    }

    #[test]
    fn leaves_unknown_or_missing_windows_unavailable() {
        let rate_limit = RateLimitDetails {
            primary_window: Some(window(10.0, 3_600, 1_800_000_000)),
            secondary_window: None,
        };
        let mapped = classify_windows(Some(rate_limit));
        assert!(mapped.five_hour.is_none());
        assert!(mapped.seven_day.is_none());
    }

    #[test]
    fn clamps_invalid_percentages() {
        let mapped = quota_window(window(120.0, FIVE_HOURS_SECONDS, 1_800_000_000)).unwrap();
        assert_eq!(mapped.used_percent, 100.0);
        assert_eq!(mapped.remaining_percent, 0.0);
    }
}
