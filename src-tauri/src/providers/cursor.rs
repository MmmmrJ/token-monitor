use crate::snapshot::{
    connected_state, provider_failure, AccountSummary, MonitorSnapshot, ProviderFailure,
    ProviderStatus, QuotaWindow, QuotaWindows,
};
use chrono::{DateTime, Utc};
use reqwest::{redirect::Policy, Client, StatusCode};
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde::Deserialize;
use std::{
    env,
    path::{Path, PathBuf},
    time::Duration,
};

const CURSOR_API_BASE: &str = "https://api2.cursor.sh";
const CURSOR_OAUTH_CLIENT_ID: &str = "KbZUR41cY7W6zRSdpSUJ7I7mLYBKOCmB";
const USAGE_PATH: &str = "/aiserver.v1.DashboardService/GetCurrentPeriodUsage";
const PLAN_INFO_PATH: &str = "/aiserver.v1.DashboardService/GetPlanInfo";
const OAUTH_TOKEN_PATH: &str = "/oauth/token";

#[derive(Clone, Debug, Default)]
struct CursorAuth {
    access_token: String,
    refresh_token: Option<String>,
    email: Option<String>,
    membership: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PeriodUsageResponse {
    billing_cycle_start: Option<String>,
    billing_cycle_end: Option<String>,
    plan_usage: Option<PlanUsage>,
    #[allow(dead_code)]
    spend_limit_usage: Option<SpendLimitUsage>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanUsage {
    /// First-party / Auto pool (Dashboard "First-party models").
    auto_percent_used: Option<f64>,
    /// API pool (Dashboard "API").
    api_percent_used: Option<f64>,
}

/// On-demand spend caps; kept for response shape, not mapped into dual rings.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SpendLimitUsage {
    individual_limit: Option<f64>,
    individual_used: Option<f64>,
    pooled_limit: Option<f64>,
    pooled_used: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanInfoResponse {
    plan_info: Option<PlanInfo>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanInfo {
    plan_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: Option<String>,
    #[serde(alias = "shouldLogout")]
    should_logout: Option<bool>,
}

fn cursor_state_db_path() -> Result<PathBuf, ProviderFailure> {
    if cfg!(target_os = "windows") {
        let appdata = env::var_os("APPDATA").ok_or_else(|| {
            provider_failure(
                "auth_missing",
                "Could not locate APPDATA for the Cursor session database.",
            )
        })?;
        Ok(PathBuf::from(appdata)
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb"))
    } else if cfg!(target_os = "macos") {
        let home = env::var_os("HOME").ok_or_else(|| {
            provider_failure(
                "auth_missing",
                "Could not locate the current user's home directory.",
            )
        })?;
        Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb"))
    } else {
        let home = env::var_os("HOME").ok_or_else(|| {
            provider_failure(
                "auth_missing",
                "Could not locate the current user's home directory.",
            )
        })?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("Cursor")
            .join("User")
            .join("globalStorage")
            .join("state.vscdb"))
    }
}

pub fn auth_path_label() -> String {
    if cfg!(target_os = "windows") {
        "%APPDATA%\\Cursor\\User\\globalStorage\\state.vscdb".into()
    } else if cfg!(target_os = "macos") {
        "~/Library/Application Support/Cursor/User/globalStorage/state.vscdb".into()
    } else {
        "~/.config/Cursor/User/globalStorage/state.vscdb".into()
    }
}

fn read_item(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row(
        "SELECT value FROM ItemTable WHERE key = ?1",
        [key],
        |row| match row.get_ref(0)? {
            ValueRef::Text(bytes) | ValueRef::Blob(bytes) => {
                Ok(String::from_utf8_lossy(bytes).into_owned())
            }
            _ => Ok(String::new()),
        },
    )
    .ok()
    .filter(|value| !value.trim().is_empty())
}

fn read_cursor_auth(db_path: &Path) -> Result<CursorAuth, ProviderFailure> {
    if !db_path.exists() {
        return Err(provider_failure(
            "auth_missing",
            "Cursor is not signed in on this computer. Sign in to Cursor first.",
        ));
    }

    let conn =
        Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|_| {
            provider_failure(
                "auth_unreadable",
                "The local Cursor session database could not be opened.",
            )
        })?;

    let access_token = read_item(&conn, "cursorAuth/accessToken").ok_or_else(|| {
        provider_failure(
            "auth_missing",
            "No Cursor access token was found. Sign in to Cursor first.",
        )
    })?;

    Ok(CursorAuth {
        access_token,
        refresh_token: read_item(&conn, "cursorAuth/refreshToken"),
        email: read_item(&conn, "cursorAuth/cachedEmail"),
        membership: read_item(&conn, "cursorAuth/stripeMembershipType"),
    })
}

fn parse_millis_timestamp(raw: Option<&str>) -> Option<DateTime<Utc>> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(ms) = raw.parse::<i64>() {
        return DateTime::from_timestamp_millis(ms);
    }
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

fn cycle_duration_seconds(start: Option<&str>, end: Option<&str>) -> i64 {
    match (parse_millis_timestamp(start), parse_millis_timestamp(end)) {
        (Some(start), Some(end)) => (end - start).num_seconds().max(0),
        _ => 0,
    }
}

fn reset_after_seconds(end: Option<&str>) -> Option<i64> {
    let end = parse_millis_timestamp(end)?;
    Some((end - Utc::now()).num_seconds().max(0))
}

fn resets_at_rfc3339(end: Option<&str>) -> Option<String> {
    parse_millis_timestamp(end).map(|dt| dt.to_rfc3339())
}

fn map_period_usage(payload: &PeriodUsageResponse) -> QuotaWindows {
    let duration = cycle_duration_seconds(
        payload.billing_cycle_start.as_deref(),
        payload.billing_cycle_end.as_deref(),
    );
    let reset_after = reset_after_seconds(payload.billing_cycle_end.as_deref());
    let resets_at = resets_at_rfc3339(payload.billing_cycle_end.as_deref());

    let mut windows = QuotaWindows::default();

    let Some(plan) = &payload.plan_usage else {
        return windows;
    };

    // Dual rings align with Dashboard: First-party (left) + API (right).
    if let Some(percent) = plan.auto_percent_used {
        windows.five_hour = Some(QuotaWindow::from_percent(
            percent,
            duration,
            reset_after,
            resets_at.clone(),
        ));
    }
    if let Some(percent) = plan.api_percent_used {
        windows.seven_day = Some(QuotaWindow::from_percent(
            percent,
            duration,
            reset_after,
            resets_at,
        ));
    }

    windows
}

fn build_client() -> Result<Client, ProviderFailure> {
    Client::builder()
        .no_gzip()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| {
            provider_failure(
                "network_error",
                "Could not initialize the secure usage connection.",
            )
        })
}

async fn refresh_access_token(
    client: &Client,
    refresh_token: &str,
) -> Result<String, ProviderFailure> {
    let response = client
        .post(format!("{CURSOR_API_BASE}{OAUTH_TOKEN_PATH}"))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": CURSOR_OAUTH_CLIENT_ID,
            "refresh_token": refresh_token,
        }))
        .send()
        .await
        .map_err(|_| {
            provider_failure(
                "network_error",
                "The Cursor auth service could not be reached. Check the network and try again.",
            )
        })?;

    if !response.status().is_success() {
        return Err(provider_failure(
            "reauth_required",
            "The Cursor session could not be refreshed. Sign in to Cursor again.",
        ));
    }

    let payload: OAuthTokenResponse = response.json().await.map_err(|_| {
        provider_failure(
            "invalid_response",
            "The Cursor auth response could not be understood.",
        )
    })?;

    if payload.should_logout.unwrap_or(false) {
        return Err(provider_failure(
            "reauth_required",
            "The Cursor session is no longer valid. Sign in to Cursor again.",
        ));
    }

    payload
        .access_token
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| {
            provider_failure(
                "reauth_required",
                "The Cursor session could not be refreshed. Sign in to Cursor again.",
            )
        })
}

async fn post_dashboard_json(
    client: &Client,
    path: &str,
    access_token: &str,
) -> Result<reqwest::Response, ProviderFailure> {
    client
        .post(format!("{CURSOR_API_BASE}{path}"))
        .bearer_auth(access_token)
        .header("Content-Type", "application/json")
        .header("Connect-Protocol-Version", "1")
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|_| {
            provider_failure(
                "network_error",
                "The Cursor usage service could not be reached. Check the network and try again.",
            )
        })
}

async fn fetch_with_token(
    client: &Client,
    access_token: &str,
) -> Result<(QuotaWindows, Option<String>), ProviderFailure> {
    let response = post_dashboard_json(client, USAGE_PATH, access_token).await?;
    if matches!(
        response.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        return Err(provider_failure(
            "reauth_required",
            "The Cursor session has expired or is not authorized. Sign in to Cursor again.",
        ));
    }
    if !response.status().is_success() {
        return Err(provider_failure(
            "service_error",
            format!(
                "The Cursor usage service returned HTTP {}.",
                response.status().as_u16()
            ),
        ));
    }

    let payload: PeriodUsageResponse = response.json().await.map_err(|_| {
        provider_failure(
            "invalid_response",
            "The Cursor usage response could not be understood.",
        )
    })?;
    let windows = map_period_usage(&payload);
    let plan_name = fetch_plan_name(client, access_token).await;
    Ok((windows, plan_name))
}

async fn fetch_plan_name(client: &Client, access_token: &str) -> Option<String> {
    let response = post_dashboard_json(client, PLAN_INFO_PATH, access_token)
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let payload: PlanInfoResponse = response.json().await.ok()?;
    payload
        .plan_info
        .and_then(|info| info.plan_name)
        .filter(|name| !name.trim().is_empty())
}

pub async fn fetch_local_snapshot() -> Result<MonitorSnapshot, ProviderFailure> {
    let label = auth_path_label();
    let mut auth = read_cursor_auth(&cursor_state_db_path()?)?;
    let client = build_client()?;

    let result = fetch_with_token(&client, &auth.access_token).await;
    let (windows, plan_name) = match result {
        Ok(value) => value,
        Err(failure)
            if failure.error_kind == crate::snapshot::ProviderErrorKind::ReauthRequired =>
        {
            let refresh = auth.refresh_token.as_deref().ok_or(failure)?;
            let new_token = refresh_access_token(&client, refresh).await?;
            auth.access_token = new_token;
            fetch_with_token(&client, &auth.access_token).await?
        }
        Err(failure) => return Err(failure),
    };

    let availability = connected_state(&windows);
    let now = Utc::now().to_rfc3339();
    Ok(MonitorSnapshot {
        account: AccountSummary {
            display_name: auth.email.unwrap_or_else(|| "Local Cursor account".into()),
            plan: plan_name
                .or(auth.membership)
                .unwrap_or_else(|| "Cursor".into()),
        },
        provider: ProviderStatus {
            kind: "cursor".into(),
            source: "local_cursor_session".into(),
            auth_path_label: label,
            availability,
            error_kind: None,
        },
        windows,
        refreshed_at: Some(now.clone()),
        checked_at: now,
        cached: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_first_party_and_api_percent() {
        let payload = PeriodUsageResponse {
            billing_cycle_start: Some("1768399334000".into()),
            billing_cycle_end: Some("1771077734000".into()),
            plan_usage: Some(PlanUsage {
                auto_percent_used: Some(32.0),
                api_percent_used: Some(22.0),
            }),
            spend_limit_usage: Some(SpendLimitUsage {
                individual_limit: Some(0.0),
                individual_used: Some(0.0),
                pooled_limit: None,
                pooled_used: None,
            }),
        };
        let windows = map_period_usage(&payload);
        let first_party = windows.five_hour.unwrap();
        assert_eq!(first_party.used_percent, 32.0);
        assert_eq!(first_party.remaining_percent, 68.0);
        assert!(first_party.unit.is_none());
        let api = windows.seven_day.unwrap();
        assert_eq!(api.used_percent, 22.0);
        assert_eq!(api.remaining_percent, 78.0);
    }

    #[test]
    fn omits_missing_percent_pools() {
        let payload = PeriodUsageResponse {
            billing_cycle_start: Some("1768399334000".into()),
            billing_cycle_end: Some("1771077734000".into()),
            plan_usage: Some(PlanUsage {
                auto_percent_used: Some(10.0),
                api_percent_used: None,
            }),
            spend_limit_usage: None,
        };
        let windows = map_period_usage(&payload);
        assert_eq!(windows.five_hour.unwrap().used_percent, 10.0);
        assert!(windows.seven_day.is_none());
    }

    #[test]
    fn does_not_fall_back_to_money_or_total_percent() {
        let payload: PeriodUsageResponse = serde_json::from_value(serde_json::json!({
            "billingCycleStart": "1768399334000",
            "billingCycleEnd": "1771077734000",
            "planUsage": {
                "includedSpend": 23222.0,
                "remaining": 16778.0,
                "limit": 40000.0,
                "totalPercentUsed": 58.055
            },
            "spendLimitUsage": {
                "individualLimit": 10000.0,
                "individualUsed": 5000.0
            }
        }))
        .expect("deserialize unrelated Cursor spend fields");
        let windows = map_period_usage(&payload);
        assert!(windows.five_hour.is_none());
        assert!(windows.seven_day.is_none());
    }

    #[test]
    fn does_not_map_legacy_request_buckets() {
        let payload: PeriodUsageResponse = serde_json::from_value(serde_json::json!({
            "gpt-4": { "numRequests": 150, "maxRequestUsage": 500 },
            "startOfMonth": "2026-03-01T00:00:00.000Z"
        }))
        .expect("deserialize legacy-only response as an empty dashboard response");
        let windows = map_period_usage(&payload);
        assert!(windows.five_hour.is_none());
        assert!(windows.seven_day.is_none());
    }

    #[test]
    fn preserves_missing_reset_time_as_null() {
        let payload = PeriodUsageResponse {
            billing_cycle_start: None,
            billing_cycle_end: None,
            plan_usage: Some(PlanUsage {
                auto_percent_used: Some(15.0),
                api_percent_used: None,
            }),
            spend_limit_usage: None,
        };
        let windows = map_period_usage(&payload);
        let primary = windows.five_hour.expect("first-party window");
        assert!(primary.resets_at.is_none());
        assert!(primary.reset_after_seconds.is_none());
    }

    #[test]
    fn rejects_target_field_type_changes() {
        let result = serde_json::from_value::<PeriodUsageResponse>(serde_json::json!({
            "planUsage": {
                "autoPercentUsed": "32 percent",
                "apiPercentUsed": 22.0
            }
        }));
        assert!(result.is_err());
    }
}
