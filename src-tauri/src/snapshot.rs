use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

pub const SOURCE_LABEL_CODEX: &str = "Codex auth.json";
pub const SOURCE_LABEL_CURSOR: &str = "Cursor local session";

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindows {
    pub five_hour: Option<QuotaWindow>,
    pub seven_day: Option<QuotaWindow>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub used_percent: f64,
    pub remaining_percent: f64,
    pub duration_seconds: i64,
    pub reset_after_seconds: Option<i64>,
    pub resets_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit_amount: Option<f64>,
}

impl QuotaWindow {
    pub fn from_percent(
        used_percent: f64,
        duration_seconds: i64,
        reset_after_seconds: Option<i64>,
        resets_at: Option<String>,
    ) -> Self {
        let used = used_percent.clamp(0.0, 100.0);
        Self {
            used_percent: used,
            remaining_percent: 100.0 - used,
            duration_seconds,
            reset_after_seconds,
            resets_at,
            unit: None,
            used_amount: None,
            limit_amount: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAvailability {
    Live,
    Partial,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorKind {
    AuthMissing,
    AuthUnreadable,
    AuthInvalid,
    ReauthRequired,
    UnsupportedAuth,
    NetworkError,
    ServiceError,
    InvalidResponse,
}

impl ProviderErrorKind {
    fn from_code(code: &str) -> Self {
        match code {
            "auth_missing" => Self::AuthMissing,
            "auth_unreadable" => Self::AuthUnreadable,
            "auth_invalid" => Self::AuthInvalid,
            "reauth_required" => Self::ReauthRequired,
            "unsupported_auth" => Self::UnsupportedAuth,
            "network_error" => Self::NetworkError,
            "invalid_response" => Self::InvalidResponse,
            _ => Self::ServiceError,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub kind: String,
    pub source: String,
    pub source_label: String,
    pub availability: ProviderAvailability,
    pub error_kind: Option<ProviderErrorKind>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorSnapshot {
    pub provider: ProviderStatus,
    pub windows: QuotaWindows,
    pub refreshed_at: Option<String>,
    pub checked_at: String,
    pub cached: bool,
}

#[derive(Default)]
pub struct MonitorState {
    pub last_provider: Mutex<String>,
    pub snapshots: Mutex<HashMap<String, MonitorSnapshot>>,
}

#[derive(Debug)]
pub struct ProviderFailure {
    pub error_kind: ProviderErrorKind,
}

pub fn provider_failure(code: &'static str, _message: impl Into<String>) -> ProviderFailure {
    ProviderFailure {
        error_kind: ProviderErrorKind::from_code(code),
    }
}

pub fn normalize_provider(provider: Option<&str>) -> &'static str {
    match provider.map(str::trim).unwrap_or("codex") {
        "cursor" => "cursor",
        _ => "codex",
    }
}

pub fn fixed_source_label(kind: &str) -> &'static str {
    match normalize_provider(Some(kind)) {
        "cursor" => SOURCE_LABEL_CURSOR,
        _ => SOURCE_LABEL_CODEX,
    }
}

pub fn failure_snapshot(kind: &str, failure: ProviderFailure) -> MonitorSnapshot {
    let source = match kind {
        "cursor" => "local_cursor_session",
        _ => "local_codex_oauth",
    };
    MonitorSnapshot {
        provider: ProviderStatus {
            kind: kind.into(),
            source: source.into(),
            source_label: fixed_source_label(kind).into(),
            availability: ProviderAvailability::Unavailable,
            error_kind: Some(failure.error_kind),
        },
        windows: QuotaWindows::default(),
        refreshed_at: None,
        checked_at: Utc::now().to_rfc3339(),
        cached: false,
    }
}

pub fn connected_state(windows: &QuotaWindows) -> ProviderAvailability {
    match (windows.five_hour.is_some(), windows.seven_day.is_some()) {
        (true, true) => ProviderAvailability::Live,
        (false, false) => ProviderAvailability::Unavailable,
        _ => ProviderAvailability::Partial,
    }
}

pub fn cached_failure_snapshot(
    mut cached: MonitorSnapshot,
    failure: ProviderFailure,
) -> MonitorSnapshot {
    cached.provider.error_kind = Some(failure.error_kind);
    cached.checked_at = Utc::now().to_rfc3339();
    cached.cached = true;
    cached
}

fn json_text_contains_sensitive(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains('@')
        || lower.contains("bearer ")
        || lower.contains("eyj")
        || lower.contains("/users/")
        || lower.contains("/home/")
        || lower.contains("c:\\")
        || lower.contains("%appdata%")
        || lower.contains("://")
        || lower.contains("displayname")
        || lower.contains("authpathlabel")
        || lower.contains("\"plan\"")
}

pub fn assert_snapshot_safe_for_webview(snapshot: &MonitorSnapshot) -> Result<(), String> {
    let value = serde_json::to_value(snapshot).map_err(|error| error.to_string())?;
    let text = value.to_string();
    if json_text_contains_sensitive(&text) {
        return Err("snapshot_contains_sensitive_data".into());
    }
    if value.get("account").is_some() {
        return Err("snapshot_contains_account".into());
    }
    let provider = value
        .get("provider")
        .ok_or_else(|| "snapshot_missing_provider".to_string())?;
    for key in ["displayName", "plan", "authPathLabel", "token", "accountId"] {
        if provider.get(key).is_some() || value.get(key).is_some() {
            return Err(format!("snapshot_contains_forbidden_key:{key}"));
        }
    }
    let label = provider
        .get("sourceLabel")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if label != SOURCE_LABEL_CODEX && label != SOURCE_LABEL_CURSOR {
        return Err("snapshot_invalid_source_label".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_failure_preserves_last_success_metadata() {
        let snapshot = MonitorSnapshot {
            provider: ProviderStatus {
                kind: "codex".into(),
                source: "local_codex_oauth".into(),
                source_label: SOURCE_LABEL_CODEX.into(),
                availability: ProviderAvailability::Live,
                error_kind: None,
            },
            windows: QuotaWindows::default(),
            refreshed_at: Some("2026-07-15T00:00:00Z".into()),
            checked_at: "2026-07-15T00:00:00Z".into(),
            cached: false,
        };

        let cached = cached_failure_snapshot(
            snapshot,
            provider_failure("network_error", "network unavailable"),
        );

        assert_eq!(cached.provider.availability, ProviderAvailability::Live);
        assert_eq!(
            cached.provider.error_kind,
            Some(ProviderErrorKind::NetworkError)
        );
        assert_eq!(cached.refreshed_at.as_deref(), Some("2026-07-15T00:00:00Z"));
        assert!(cached.cached);
    }

    #[test]
    fn window_availability_distinguishes_partial_and_unavailable() {
        let mut windows = QuotaWindows::default();
        assert_eq!(connected_state(&windows), ProviderAvailability::Unavailable);
        windows.five_hour = Some(QuotaWindow::from_percent(25.0, 18_000, None, None));
        assert_eq!(connected_state(&windows), ProviderAvailability::Partial);
    }

    #[test]
    fn serialized_status_uses_the_orthogonal_contract() {
        let snapshot = failure_snapshot(
            "cursor",
            provider_failure("network_error", "network unavailable"),
        );
        let value = serde_json::to_value(&snapshot).expect("serialize failure snapshot");
        let provider = value.get("provider").expect("provider status");

        assert_eq!(
            provider
                .get("availability")
                .and_then(|value| value.as_str()),
            Some("unavailable")
        );
        assert_eq!(
            provider.get("errorKind").and_then(|value| value.as_str()),
            Some("network_error")
        );
        assert!(provider.get("state").is_none());
        assert!(provider.get("connected").is_none());
        assert_eq!(
            provider.get("sourceLabel").and_then(|value| value.as_str()),
            Some(SOURCE_LABEL_CURSOR)
        );
        assert!(assert_snapshot_safe_for_webview(&snapshot).is_ok());
    }

    #[test]
    fn serialized_snapshot_rejects_identity_and_path_leakage() {
        let snapshot = MonitorSnapshot {
            provider: ProviderStatus {
                kind: "codex".into(),
                source: "local_codex_oauth".into(),
                source_label: SOURCE_LABEL_CODEX.into(),
                availability: ProviderAvailability::Live,
                error_kind: None,
            },
            windows: QuotaWindows::default(),
            refreshed_at: Some("2026-07-27T00:00:00Z".into()),
            checked_at: "2026-07-27T00:00:00Z".into(),
            cached: false,
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(!json.contains("displayName"));
        assert!(!json.contains("authPathLabel"));
        assert!(!json.contains("account"));
        assert!(!json.contains('@'));
        assert!(!json.contains("/Users/"));
        assert!(assert_snapshot_safe_for_webview(&snapshot).is_ok());
    }
}
