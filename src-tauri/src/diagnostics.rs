use crate::snapshot::{
    normalize_provider, MonitorSnapshot, MonitorState, ProviderAvailability, ProviderErrorKind,
};
use serde::Serialize;
use tauri::{AppHandle, State};

const RELEASE_URL: &str = "https://github.com/MmmmrJ/token-monitor/releases";
const ISSUE_URL: &str = "https://github.com/MmmmrJ/token-monitor/issues";
const SOURCE_CODEX: &str = "Codex auth.json";
const SOURCE_CURSOR: &str = "Cursor local session";

const FORBIDDEN_KEYS: &[&str] = &[
    "token",
    "accessToken",
    "refreshToken",
    "idToken",
    "authorization",
    "password",
    "accountId",
    "chatgptAccountId",
    "authPath",
    "auth_path",
    "displayName",
    "plan",
    "remainingPercent",
    "usedPercent",
    "usedAmount",
    "limitAmount",
    "url",
    "endpoint",
    "baseUrl",
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub release_url: String,
    pub issue_url: String,
    pub updater_signature_status: String,
    pub platform_signing_status: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeDiagnostics {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub provider: String,
    pub availability: String,
    pub error_kind: Option<String>,
    pub cached: bool,
    pub checked_at: Option<String>,
    pub refreshed_at: Option<String>,
    pub has_five_hour: bool,
    pub has_seven_day: bool,
    pub source_label: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsCopyResult {
    pub ok: bool,
    pub text: Option<String>,
    pub error: Option<String>,
}

fn current_os() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Other"
    }
}

fn current_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else {
        "unknown"
    }
}

fn platform_signing_status() -> &'static str {
    // Public Stable builds intentionally ship without Apple notarization / Authenticode.
    "not_configured"
}

fn updater_signature_status() -> &'static str {
    "configured"
}

fn source_label(provider: &str) -> &'static str {
    match normalize_provider(Some(provider)) {
        "cursor" => SOURCE_CURSOR,
        _ => SOURCE_CODEX,
    }
}

fn availability_label(value: ProviderAvailability) -> &'static str {
    match value {
        ProviderAvailability::Live => "live",
        ProviderAvailability::Partial => "partial",
        ProviderAvailability::Unavailable => "unavailable",
    }
}

fn error_kind_label(value: ProviderErrorKind) -> &'static str {
    match value {
        ProviderErrorKind::AuthMissing => "auth_missing",
        ProviderErrorKind::AuthUnreadable => "auth_unreadable",
        ProviderErrorKind::AuthInvalid => "auth_invalid",
        ProviderErrorKind::ReauthRequired => "reauth_required",
        ProviderErrorKind::UnsupportedAuth => "unsupported_auth",
        ProviderErrorKind::NetworkError => "network_error",
        ProviderErrorKind::ServiceError => "service_error",
        ProviderErrorKind::InvalidResponse => "invalid_response",
    }
}

pub fn build_app_info(version: impl Into<String>) -> AppInfo {
    AppInfo {
        version: version.into(),
        os: current_os().into(),
        arch: current_arch().into(),
        release_url: RELEASE_URL.into(),
        issue_url: ISSUE_URL.into(),
        updater_signature_status: updater_signature_status().into(),
        platform_signing_status: platform_signing_status().into(),
    }
}

pub fn safe_diagnostics_from_snapshot(
    version: impl Into<String>,
    provider: &str,
    snapshot: Option<&MonitorSnapshot>,
) -> SafeDiagnostics {
    let provider = normalize_provider(Some(provider)).to_string();
    let Some(snapshot) = snapshot else {
        let source = source_label(&provider).to_string();
        return SafeDiagnostics {
            version: version.into(),
            os: current_os().into(),
            arch: current_arch().into(),
            provider,
            availability: "unavailable".into(),
            error_kind: None,
            cached: false,
            checked_at: None,
            refreshed_at: None,
            has_five_hour: false,
            has_seven_day: false,
            source_label: source,
        };
    };

    SafeDiagnostics {
        version: version.into(),
        os: current_os().into(),
        arch: current_arch().into(),
        provider: snapshot.provider.kind.clone(),
        availability: availability_label(snapshot.provider.availability).into(),
        error_kind: snapshot
            .provider
            .error_kind
            .map(error_kind_label)
            .map(str::to_string),
        cached: snapshot.cached,
        checked_at: Some(snapshot.checked_at.clone()),
        refreshed_at: snapshot.refreshed_at.clone(),
        has_five_hour: snapshot.windows.five_hour.is_some(),
        has_seven_day: snapshot.windows.seven_day.is_some(),
        source_label: source_label(&snapshot.provider.kind).into(),
    }
}

fn contains_forbidden_pattern(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    if lower.contains("bearer ")
        || lower.contains("authorization:")
        || text.contains("eyJ")
        || lower.contains("sk-")
        || lower.contains("://")
        || text.contains("/Users/")
        || text.contains("/home/")
        || text.contains("C:\\")
        || text.contains("%APPDATA%")
        || text.contains("\\\\")
    {
        return true;
    }
    // Reject quota-like numeric percents embedded in diagnostic text.
    if text.chars().any(|ch| ch == '%') {
        return true;
    }
    false
}

fn json_contains_forbidden(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                if FORBIDDEN_KEYS
                    .iter()
                    .any(|forbidden| key.eq_ignore_ascii_case(forbidden))
                {
                    return true;
                }
                if json_contains_forbidden(child) {
                    return true;
                }
            }
            false
        }
        serde_json::Value::Array(items) => items.iter().any(json_contains_forbidden),
        serde_json::Value::String(text) => contains_forbidden_pattern(text),
        _ => false,
    }
}

pub fn assert_diagnostics_safe(diagnostics: &SafeDiagnostics) -> Result<(), String> {
    let value = serde_json::to_value(diagnostics).map_err(|error| error.to_string())?;
    if json_contains_forbidden(&value) {
        return Err("diagnostics_blocked".into());
    }
    let allowed = [SOURCE_CODEX, SOURCE_CURSOR];
    if !allowed.contains(&diagnostics.source_label.as_str()) {
        return Err("diagnostics_blocked".into());
    }
    Ok(())
}

pub fn format_diagnostics_text(diagnostics: &SafeDiagnostics, language: &str) -> String {
    let zh = language != "en";
    let mut lines = Vec::new();
    if zh {
        lines.push("Token Monitor 诊断摘要".into());
        lines.push(format!("版本：{}", diagnostics.version));
        lines.push(format!("系统：{} ({})", diagnostics.os, diagnostics.arch));
        lines.push(format!("Provider：{}", diagnostics.provider));
        lines.push(format!("可用性：{}", diagnostics.availability));
        lines.push(format!(
            "错误类型：{}",
            diagnostics.error_kind.as_deref().unwrap_or("无")
        ));
        lines.push(format!(
            "缓存：{}",
            if diagnostics.cached { "是" } else { "否" }
        ));
        lines.push(format!(
            "检查时间：{}",
            diagnostics.checked_at.as_deref().unwrap_or("—")
        ));
        lines.push(format!(
            "刷新时间：{}",
            diagnostics.refreshed_at.as_deref().unwrap_or("—")
        ));
        lines.push(format!(
            "主窗口：{}",
            if diagnostics.has_five_hour {
                "有"
            } else {
                "无"
            }
        ));
        lines.push(format!(
            "次窗口：{}",
            if diagnostics.has_seven_day {
                "有"
            } else {
                "无"
            }
        ));
        lines.push(format!("来源标签：{}", diagnostics.source_label));
    } else {
        lines.push("Token Monitor diagnostics".into());
        lines.push(format!("Version: {}", diagnostics.version));
        lines.push(format!("OS: {} ({})", diagnostics.os, diagnostics.arch));
        lines.push(format!("Provider: {}", diagnostics.provider));
        lines.push(format!("Availability: {}", diagnostics.availability));
        lines.push(format!(
            "Error kind: {}",
            diagnostics.error_kind.as_deref().unwrap_or("none")
        ));
        lines.push(format!("Cached: {}", diagnostics.cached));
        lines.push(format!(
            "Checked at: {}",
            diagnostics.checked_at.as_deref().unwrap_or("—")
        ));
        lines.push(format!(
            "Refreshed at: {}",
            diagnostics.refreshed_at.as_deref().unwrap_or("—")
        ));
        lines.push(format!("Has primary window: {}", diagnostics.has_five_hour));
        lines.push(format!(
            "Has secondary window: {}",
            diagnostics.has_seven_day
        ));
        lines.push(format!("Source label: {}", diagnostics.source_label));
    }
    lines.join("\n")
}

#[tauri::command]
pub fn get_app_info(app: AppHandle) -> AppInfo {
    build_app_info(app.package_info().version.to_string())
}

#[tauri::command]
pub fn get_safe_diagnostics(
    app: AppHandle,
    state: State<'_, MonitorState>,
    provider: Option<String>,
    language: Option<String>,
) -> Result<DiagnosticsCopyResult, String> {
    let provider = normalize_provider(provider.as_deref()).to_string();
    let snapshot = state
        .snapshots
        .lock()
        .map_err(|_| "diagnostics_unavailable".to_string())?
        .get(&provider)
        .cloned();
    let diagnostics = safe_diagnostics_from_snapshot(
        app.package_info().version.to_string(),
        &provider,
        snapshot.as_ref(),
    );
    if let Err(error) = assert_diagnostics_safe(&diagnostics) {
        return Ok(DiagnosticsCopyResult {
            ok: false,
            text: None,
            error: Some(error),
        });
    }
    let lang = language.unwrap_or_else(|| "zh".into());
    let text = format_diagnostics_text(&diagnostics, &lang);
    if contains_forbidden_pattern(&text) {
        return Ok(DiagnosticsCopyResult {
            ok: false,
            text: None,
            error: Some("diagnostics_blocked".into()),
        });
    }
    Ok(DiagnosticsCopyResult {
        ok: true,
        text: Some(text),
        error: None,
    })
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    let allowed = [RELEASE_URL, ISSUE_URL];
    if !allowed
        .iter()
        .any(|item| url == *item || url.starts_with(&format!("{item}/")))
    {
        return Err("url_not_allowed".into());
    }
    open_url(&url)
}

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|error| error.to_string())?;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map_err(|error| error.to_string())?;
        Ok(())
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = url;
        Err("unsupported_platform".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{ProviderStatus, QuotaWindows, SOURCE_LABEL_CODEX};

    fn sample_snapshot() -> MonitorSnapshot {
        MonitorSnapshot {
            provider: ProviderStatus {
                kind: "codex".into(),
                source: "local_codex_oauth".into(),
                source_label: SOURCE_LABEL_CODEX.into(),
                availability: ProviderAvailability::Partial,
                error_kind: Some(ProviderErrorKind::NetworkError),
            },
            windows: QuotaWindows {
                five_hour: None,
                seven_day: Some(crate::snapshot::QuotaWindow::from_percent(
                    42.0,
                    604_800,
                    Some(10),
                    None,
                )),
            },
            refreshed_at: Some("2026-07-27T01:00:00Z".into()),
            checked_at: "2026-07-27T01:05:00Z".into(),
            cached: true,
        }
    }

    #[test]
    fn app_info_maps_platform_fields() {
        let info = build_app_info("1.3.0");
        assert_eq!(info.version, "1.3.0");
        assert_eq!(info.release_url, RELEASE_URL);
        assert_eq!(info.issue_url, ISSUE_URL);
        assert_eq!(info.updater_signature_status, "configured");
        assert_eq!(info.platform_signing_status, "not_configured");
        assert!(!info.os.is_empty());
        assert!(!info.arch.is_empty());
    }

    #[test]
    fn diagnostics_whitelist_omits_sensitive_snapshot_fields() {
        let diagnostics =
            safe_diagnostics_from_snapshot("1.3.0", "codex", Some(&sample_snapshot()));
        assert_eq!(diagnostics.source_label, SOURCE_CODEX);
        assert_eq!(diagnostics.availability, "partial");
        assert_eq!(diagnostics.error_kind.as_deref(), Some("network_error"));
        assert!(diagnostics.has_seven_day);
        assert!(!diagnostics.has_five_hour);
        assert!(assert_diagnostics_safe(&diagnostics).is_ok());
        let text = format_diagnostics_text(&diagnostics, "zh");
        assert!(!text.contains("secret@"));
        assert!(!text.contains("/Users/"));
        assert!(!text.contains("42"));
        assert!(!text.contains('%'));
    }

    #[test]
    fn diagnostics_reject_custom_source_labels_and_forbidden_patterns() {
        let mut diagnostics =
            safe_diagnostics_from_snapshot("1.3.0", "cursor", Some(&sample_snapshot()));
        diagnostics.source_label = "/tmp/evil".into();
        assert!(assert_diagnostics_safe(&diagnostics).is_err());

        diagnostics.source_label = SOURCE_CURSOR.into();
        diagnostics.checked_at = Some("https://evil.example".into());
        assert!(assert_diagnostics_safe(&diagnostics).is_err());

        diagnostics.checked_at = Some("2026-07-27T01:05:00Z".into());
        diagnostics.error_kind = Some("Bearer eyJabc".into());
        assert!(assert_diagnostics_safe(&diagnostics).is_err());
    }
}
