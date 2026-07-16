use crate::snapshot::{
    connected_state, provider_failure, AccountSummary, MonitorSnapshot, ProviderFailure,
    ProviderStatus, QuotaWindow, QuotaWindows,
};
use base64::{
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use chrono::Utc;
use reqwest::{redirect::Policy, Client, StatusCode, Url};
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
    pub reset_at: Option<i64>,
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

fn default_usage_url() -> Url {
    Url::parse(DEFAULT_USAGE_URL).expect("the built-in Codex usage URL must be valid")
}

fn validated_usage_url(raw_base: &str) -> Option<Url> {
    let mut url = Url::parse(raw_base.trim()).ok()?;
    let allowed_host = url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("chatgpt.com") || host.eq_ignore_ascii_case("chat.openai.com")
    });
    let allowed_path = matches!(url.path(), "" | "/" | "/backend-api" | "/backend-api/");

    if url.scheme() != "https"
        || !allowed_host
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some_and(|port| port != 443)
        || url.query().is_some()
        || url.fragment().is_some()
        || !allowed_path
    {
        return None;
    }

    url.set_path("/backend-api/wham/usage");
    Some(url)
}

fn configured_usage_url(home: &Path) -> Url {
    let config = match fs::read_to_string(home.join("config.toml")) {
        Ok(value) => value,
        Err(_) => return default_usage_url(),
    };

    for raw_line in config.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "chatgpt_base_url" {
            continue;
        }
        let base = value.trim().trim_matches(['\'', '"']);
        if let Some(url) = validated_usage_url(base) {
            return url;
        }
    }

    default_usage_url()
}

fn build_client() -> Result<Client, ProviderFailure> {
    Client::builder()
        .no_gzip()
        .redirect(Policy::none())
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|_| {
            provider_failure(
                "network_error",
                "Could not initialize the secure usage connection.",
            )
        })
}

pub fn quota_window(window: UsageWindowResponse) -> QuotaWindow {
    let resets_at = window
        .reset_at
        .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
        .map(|date| date.to_rfc3339());
    QuotaWindow::from_percent(
        window.used_percent,
        window.limit_window_seconds,
        window.reset_after_seconds,
        resets_at,
    )
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
        let distance_five =
            (duration - FIVE_HOURS_SECONDS).abs() as f64 / FIVE_HOURS_SECONDS as f64;
        let distance_week =
            (duration - SEVEN_DAYS_SECONDS).abs() as f64 / SEVEN_DAYS_SECONDS as f64;
        if distance_five <= 0.10 && result.five_hour.is_none() {
            result.five_hour = Some(mapped);
        } else if distance_week <= 0.10 && result.seven_day.is_none() {
            result.seven_day = Some(mapped);
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

    let client = build_client()?;
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
    let availability = connected_state(&windows);
    let now = Utc::now().to_rfc3339();
    Ok(MonitorSnapshot {
        account,
        provider: ProviderStatus {
            kind: "codex".into(),
            source: "local_codex_oauth".into(),
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
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread,
        time::{Duration, Instant},
    };

    #[test]
    fn accepts_only_exact_codex_https_bases() {
        for base in [
            "https://chatgpt.com",
            "https://chatgpt.com/",
            "https://chatgpt.com/backend-api",
            "https://chat.openai.com/backend-api/",
            "https://CHATGPT.com:443",
        ] {
            let url = validated_usage_url(base).unwrap_or_else(|| panic!("rejected {base}"));
            assert_eq!(url.path(), "/backend-api/wham/usage");
        }

        for base in [
            "https://chatgpt.com.evil.example",
            "https://chatgpt.com@evil.example",
            "http://chatgpt.com",
            "https://chatgpt.com:444",
            "https://chat.openai.com.evil.example/backend-api",
            "https://chatgpt.com/backend-api/wham/usage",
            "https://chatgpt.com/backend-api?next=https://evil.example",
            "https://chatgpt.com/backend-api#fragment",
        ] {
            assert!(validated_usage_url(base).is_none(), "accepted {base}");
        }
    }

    #[test]
    fn configured_url_falls_back_for_untrusted_base() {
        let unique = format!(
            "token-monitor-url-test-{}-{}",
            std::process::id(),
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        );
        let home = std::env::temp_dir().join(unique);
        fs::create_dir_all(&home).expect("create temporary Codex home");
        fs::write(
            home.join("config.toml"),
            "chatgpt_base_url = \"https://chatgpt.com.evil.example\"\n",
        )
        .expect("write config");

        assert_eq!(configured_usage_url(&home).as_str(), DEFAULT_USAGE_URL);
        fs::remove_dir_all(home).expect("remove temporary Codex home");
    }

    #[test]
    fn client_never_follows_redirects() {
        for status in [301_u16, 302, 307, 308] {
            let target = TcpListener::bind("127.0.0.1:0").expect("bind redirect target");
            target
                .set_nonblocking(true)
                .expect("set redirect target nonblocking");
            let target_address = target.local_addr().expect("read redirect target address");
            let target_hit = Arc::new(AtomicBool::new(false));
            let target_hit_on_thread = Arc::clone(&target_hit);
            let target_thread = thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_millis(400);
                while Instant::now() < deadline {
                    match target.accept() {
                        Ok((mut stream, _)) => {
                            target_hit_on_thread.store(true, Ordering::SeqCst);
                            let _ = stream.write_all(
                                b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                            );
                            return;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(_) => return,
                    }
                }
            });

            let source = TcpListener::bind("127.0.0.1:0").expect("bind redirect source");
            let source_address = source.local_addr().expect("read redirect source address");
            let source_thread = thread::spawn(move || {
                let (mut stream, _) = source.accept().expect("accept source request");
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .expect("set source read timeout");
                let mut request = [0_u8; 2048];
                let _ = stream.read(&mut request);
                let response = format!(
                    "HTTP/1.1 {status} Redirect\r\nLocation: http://{target_address}/target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write redirect response");
            });

            let response = tauri::async_runtime::block_on(async {
                build_client()
                    .expect("build client")
                    .get(format!("http://{source_address}/source"))
                    .send()
                    .await
                    .expect("receive redirect response")
            });

            source_thread.join().expect("join redirect source");
            target_thread.join().expect("join redirect target");
            assert_eq!(response.status().as_u16(), status);
            assert!(!target_hit.load(Ordering::SeqCst), "followed HTTP {status}");
        }
    }
}
