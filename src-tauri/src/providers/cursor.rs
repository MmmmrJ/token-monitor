use crate::snapshot::{
    connected_state, provider_failure, AccountSummary, MonitorSnapshot, ProviderFailure,
    ProviderStatus, QuotaWindow, QuotaWindows,
};
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use rusqlite::{types::ValueRef, Connection, OpenFlags};
use serde::Deserialize;
use serde_json::Value;
use std::{
    env,
    path::{Path, PathBuf},
    time::Duration,
};

const CURSOR_API_BASE: &str = "https://api2.cursor.sh";
const CURSOR_OAUTH_CLIENT_ID: &str = "KbZUR41cY7W6zRSdpSUJ7I7mLYBKOCmB";
const USAGE_PATH: &str = "/aiserver.v1.DashboardService/GetCurrentPeriodUsage";
const PLAN_INFO_PATH: &str = "/aiserver.v1.DashboardService/GetPlanInfo";
const LEGACY_USAGE_PATH: &str = "/auth/usage";
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
    spend_limit_usage: Option<SpendLimitUsage>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanUsage {
    included_spend: Option<f64>,
    remaining: Option<f64>,
    limit: Option<f64>,
    total_percent_used: Option<f64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    let resets_at = resets_at_rfc3339(payload.billing_cycle_end.as_deref())
        .unwrap_or_else(|| Utc::now().to_rfc3339());

    let mut windows = QuotaWindows::default();

    if let Some(plan) = &payload.plan_usage {
        let limit = plan.limit.unwrap_or(0.0);
        let used = if limit > 0.0 {
            plan.included_spend
                .or_else(|| plan.remaining.map(|remaining| (limit - remaining).max(0.0)))
                .unwrap_or(0.0)
        } else {
            0.0
        };
        if let Some(window) = QuotaWindow::from_amounts(
            used,
            limit,
            duration,
            reset_after,
            resets_at.clone(),
            "usd_cents",
        ) {
            windows.five_hour = Some(window);
        } else if let Some(percent) = plan.total_percent_used {
            windows.five_hour = Some(QuotaWindow::from_percent(
                percent,
                duration,
                reset_after,
                resets_at.clone(),
            ));
        }
    }

    if let Some(spend) = &payload.spend_limit_usage {
        let (used, limit) = if spend.individual_limit.unwrap_or(0.0) > 0.0 {
            (
                spend.individual_used.unwrap_or(0.0),
                spend.individual_limit.unwrap_or(0.0),
            )
        } else if spend.pooled_limit.unwrap_or(0.0) > 0.0 {
            (
                spend.pooled_used.unwrap_or(0.0),
                spend.pooled_limit.unwrap_or(0.0),
            )
        } else {
            (0.0, 0.0)
        };
        windows.seven_day =
            QuotaWindow::from_amounts(used, limit, duration, reset_after, resets_at, "usd_cents");
    }

    windows
}

pub fn map_legacy_usage(value: &Value) -> QuotaWindows {
    let mut windows = QuotaWindows::default();
    let start = value
        .get("startOfMonth")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let resets_at = start
        .as_deref()
        .and_then(|raw| {
            DateTime::parse_from_rfc3339(raw)
                .ok()
                .map(|dt| dt.with_timezone(&Utc) + chrono::Duration::days(30))
        })
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| Utc::now().to_rfc3339());
    let reset_after = DateTime::parse_from_rfc3339(&resets_at)
        .ok()
        .map(|dt| (dt.with_timezone(&Utc) - Utc::now()).num_seconds().max(0));

    let preferred = ["gpt-4", "gpt-4o", "claude-4", "default-model"];
    let mut chosen: Option<(&str, f64, f64)> = None;

    if let Some(obj) = value.as_object() {
        for key in preferred {
            if let Some(bucket) = obj.get(key) {
                let max = bucket
                    .get("maxRequestUsage")
                    .and_then(|v| v.as_f64())
                    .or_else(|| {
                        bucket
                            .get("maxRequestUsage")
                            .and_then(|v| v.as_i64())
                            .map(|v| v as f64)
                    })
                    .unwrap_or(0.0);
                let used = bucket
                    .get("numRequests")
                    .and_then(|v| v.as_f64())
                    .or_else(|| {
                        bucket
                            .get("numRequests")
                            .and_then(|v| v.as_i64())
                            .map(|v| v as f64)
                    })
                    .unwrap_or(0.0);
                if max > 0.0 {
                    chosen = Some((key, used, max));
                    break;
                }
            }
        }
        if chosen.is_none() {
            for (key, bucket) in obj {
                if key == "startOfMonth" {
                    continue;
                }
                let max = bucket
                    .get("maxRequestUsage")
                    .and_then(|v| v.as_f64())
                    .or_else(|| {
                        bucket
                            .get("maxRequestUsage")
                            .and_then(|v| v.as_i64())
                            .map(|v| v as f64)
                    })
                    .unwrap_or(0.0);
                let used = bucket
                    .get("numRequests")
                    .and_then(|v| v.as_f64())
                    .or_else(|| {
                        bucket
                            .get("numRequests")
                            .and_then(|v| v.as_i64())
                            .map(|v| v as f64)
                    })
                    .unwrap_or(0.0);
                if max > 0.0 {
                    chosen = Some((key.as_str(), used, max));
                    break;
                }
            }
        }
    }

    if let Some((_key, used, max)) = chosen {
        windows.five_hour =
            QuotaWindow::from_amounts(used, max, 2_592_000, reset_after, resets_at, "requests");
    }
    windows
}

fn build_client() -> Result<Client, ProviderFailure> {
    Client::builder()
        .no_gzip()
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

async fn get_legacy_usage(
    client: &Client,
    access_token: &str,
) -> Result<reqwest::Response, ProviderFailure> {
    client
        .get(format!("{CURSOR_API_BASE}{LEGACY_USAGE_PATH}"))
        .bearer_auth(access_token)
        .header("Accept", "application/json")
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
    if response.status().is_success() {
        let payload: PeriodUsageResponse = response.json().await.map_err(|_| {
            provider_failure(
                "invalid_response",
                "The Cursor usage response could not be understood.",
            )
        })?;
        let windows = map_period_usage(&payload);
        if windows.five_hour.is_some() || windows.seven_day.is_some() {
            let plan_name = fetch_plan_name(client, access_token).await;
            return Ok((windows, plan_name));
        }
    } else if !matches!(
        response.status(),
        StatusCode::NOT_FOUND | StatusCode::METHOD_NOT_ALLOWED
    ) {
        return Err(provider_failure(
            "service_error",
            format!(
                "The Cursor usage service returned HTTP {}.",
                response.status().as_u16()
            ),
        ));
    }

    let legacy = get_legacy_usage(client, access_token).await?;
    if matches!(
        legacy.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        return Err(provider_failure(
            "reauth_required",
            "The Cursor session has expired or is not authorized. Sign in to Cursor again.",
        ));
    }
    if !legacy.status().is_success() {
        return Err(provider_failure(
            "service_error",
            format!(
                "The Cursor usage service returned HTTP {}.",
                legacy.status().as_u16()
            ),
        ));
    }
    let value: Value = legacy.json().await.map_err(|_| {
        provider_failure(
            "invalid_response",
            "The Cursor usage response could not be understood.",
        )
    })?;
    Ok((map_legacy_usage(&value), None))
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
        Err(failure) if failure.state == "reauth_required" => {
            let refresh = auth.refresh_token.as_deref().ok_or(failure)?;
            let new_token = refresh_access_token(&client, refresh).await?;
            auth.access_token = new_token;
            fetch_with_token(&client, &auth.access_token).await?
        }
        Err(failure) => return Err(failure),
    };

    let (state, message) = connected_state(&windows);
    let now = Utc::now().to_rfc3339();
    Ok(MonitorSnapshot {
        account: AccountSummary {
            display_name: auth.email.unwrap_or_else(|| "Local Cursor account".into()),
            plan: plan_name
                .or(auth.membership)
                .unwrap_or_else(|| "Cursor".into()),
        },
        provider: ProviderStatus {
            connected: true,
            state: state.into(),
            message,
            kind: "cursor".into(),
            source: "local_cursor_session".into(),
            auth_path_label: label,
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
    fn maps_included_and_ondemand_spend() {
        let payload = PeriodUsageResponse {
            billing_cycle_start: Some("1768399334000".into()),
            billing_cycle_end: Some("1771077734000".into()),
            plan_usage: Some(PlanUsage {
                included_spend: Some(23222.0),
                remaining: Some(16778.0),
                limit: Some(40000.0),
                total_percent_used: Some(15.48),
            }),
            spend_limit_usage: Some(SpendLimitUsage {
                individual_limit: Some(10000.0),
                individual_used: Some(2500.0),
                pooled_limit: None,
                pooled_used: None,
            }),
        };
        let windows = map_period_usage(&payload);
        let included = windows.five_hour.unwrap();
        assert!((included.used_percent - 58.055).abs() < 0.01);
        assert_eq!(included.unit.as_deref(), Some("usd_cents"));
        assert_eq!(included.used_amount, Some(23222.0));
        let ondemand = windows.seven_day.unwrap();
        assert_eq!(ondemand.used_percent, 25.0);
        assert_eq!(ondemand.limit_amount, Some(10000.0));
    }

    #[test]
    fn omits_ondemand_when_limit_missing() {
        let payload = PeriodUsageResponse {
            billing_cycle_start: Some("1768399334000".into()),
            billing_cycle_end: Some("1771077734000".into()),
            plan_usage: Some(PlanUsage {
                included_spend: Some(1000.0),
                remaining: Some(9000.0),
                limit: Some(10000.0),
                total_percent_used: None,
            }),
            spend_limit_usage: Some(SpendLimitUsage {
                individual_limit: Some(0.0),
                individual_used: Some(0.0),
                pooled_limit: Some(0.0),
                pooled_used: Some(0.0),
            }),
        };
        let windows = map_period_usage(&payload);
        assert!(windows.five_hour.is_some());
        assert!(windows.seven_day.is_none());
    }

    #[test]
    fn maps_legacy_request_buckets() {
        let value = serde_json::json!({
            "gpt-4": { "numRequests": 150, "maxRequestUsage": 500 },
            "startOfMonth": "2026-03-01T00:00:00.000Z"
        });
        let windows = map_legacy_usage(&value);
        let primary = windows.five_hour.unwrap();
        assert_eq!(primary.used_percent, 30.0);
        assert_eq!(primary.unit.as_deref(), Some("requests"));
        assert!(windows.seven_day.is_none());
    }
}
