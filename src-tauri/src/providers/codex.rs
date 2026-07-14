use crate::snapshot::{
    connected_state, provider_failure, AccountSummary, MonitorSnapshot, ProviderFailure,
    ProviderStatus, QuotaWindow, QuotaWindows,
};
use base64::{
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use chrono::Utc;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::Value;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

const DEFAULT_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
pub const FIVE_HOURS_SECONDS: i64 = 18_000;
pub const SEVEN_DAYS_SECONDS: i64 = 604_800;

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
pub struct RateLimitDetails {
    pub primary_window: Option<UsageWindowResponse>,
    pub secondary_window: Option<UsageWindowResponse>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UsageWindowResponse {
    pub used_percent: f64,
    pub limit_window_seconds: i64,
    pub reset_after_seconds: Option<i64>,
    pub reset_at: i64,
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

pub fn auth_path_label_fallback() -> String {
    codex_home()
        .ok()
        .as_deref()
        .map(auth_path_label)
        .unwrap_or_else(|| "Codex auth.json".into())
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

pub fn quota_window(window: UsageWindowResponse) -> Option<QuotaWindow> {
    let resets_at = chrono::DateTime::from_timestamp(window.reset_at, 0)?.to_rfc3339();
    Some(QuotaWindow::from_percent(
        window.used_percent,
        window.limit_window_seconds,
        window.reset_after_seconds,
        resets_at,
    ))
}

pub fn classify_windows(rate_limit: Option<RateLimitDetails>) -> QuotaWindows {
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

pub async fn fetch_local_snapshot() -> Result<MonitorSnapshot, ProviderFailure> {
    let home = codex_home()?;
    let label = auth_path_label(&home);
    let auth = read_auth(&home)?;
    if auth
        .auth_mode
        .as_deref()
        .is_some_and(|mode| !mode.eq_ignore_ascii_case("chatgpt"))
    {
        return Err(provider_failure(
            "unsupported_auth",
            "The current Codex session uses API-key mode. Sign in with ChatGPT to read subscription limits.",
        ));
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
    let (state, message) = connected_state(&windows);
    let now = Utc::now().to_rfc3339();
    Ok(MonitorSnapshot {
        account,
        provider: ProviderStatus {
            connected: true,
            state: state.into(),
            message,
            kind: "codex".into(),
            source: "local_codex_oauth".into(),
            auth_path_label: label,
        },
        windows,
        refreshed_at: Some(now.clone()),
        checked_at: now,
        cached: false,
    })
}
