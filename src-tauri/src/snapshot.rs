use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

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
    pub resets_at: String,
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
        resets_at: String,
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

    pub fn from_amounts(
        used_amount: f64,
        limit_amount: f64,
        duration_seconds: i64,
        reset_after_seconds: Option<i64>,
        resets_at: String,
        unit: &str,
    ) -> Option<Self> {
        if limit_amount <= 0.0 {
            return None;
        }
        let used = ((used_amount / limit_amount) * 100.0).clamp(0.0, 100.0);
        Some(Self {
            used_percent: used,
            remaining_percent: 100.0 - used,
            duration_seconds,
            reset_after_seconds,
            resets_at,
            unit: Some(unit.into()),
            used_amount: Some(used_amount),
            limit_amount: Some(limit_amount),
        })
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub display_name: String,
    pub plan: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub connected: bool,
    pub state: String,
    pub message: String,
    pub kind: String,
    pub source: String,
    pub auth_path_label: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorSnapshot {
    pub account: AccountSummary,
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
    pub state: &'static str,
    pub message: String,
}

pub fn provider_failure(state: &'static str, message: impl Into<String>) -> ProviderFailure {
    ProviderFailure {
        state,
        message: message.into(),
    }
}

pub fn normalize_provider(provider: Option<&str>) -> &'static str {
    match provider.map(str::trim).unwrap_or("codex") {
        "cursor" => "cursor",
        _ => "codex",
    }
}

pub fn failure_snapshot(
    kind: &str,
    failure: ProviderFailure,
    auth_path_label: String,
) -> MonitorSnapshot {
    let (display_name, source) = match kind {
        "cursor" => ("Local Cursor account", "local_cursor_session"),
        _ => ("Local Codex account", "local_codex_oauth"),
    };
    MonitorSnapshot {
        account: AccountSummary {
            display_name: display_name.into(),
            plan: "—".into(),
        },
        provider: ProviderStatus {
            connected: false,
            state: failure.state.into(),
            message: failure.message,
            kind: kind.into(),
            source: source.into(),
            auth_path_label,
        },
        windows: QuotaWindows::default(),
        refreshed_at: None,
        checked_at: Utc::now().to_rfc3339(),
        cached: false,
    }
}

pub fn connected_state(windows: &QuotaWindows) -> (&'static str, String) {
    if windows.five_hour.is_some() && windows.seven_day.is_some() {
        (
            "connected",
            "Live limits from the local sign-in session.".into(),
        )
    } else {
        (
            "partial",
            "Connected. This account currently exposes only some quota windows.".into(),
        )
    }
}
