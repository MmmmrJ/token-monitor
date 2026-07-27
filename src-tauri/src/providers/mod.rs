pub mod codex;
pub mod cursor;

use crate::snapshot::{failure_snapshot, normalize_provider, MonitorSnapshot, ProviderFailure};

pub async fn fetch_snapshot(
    provider: Option<&str>,
) -> Result<MonitorSnapshot, (String, ProviderFailure)> {
    let kind = normalize_provider(provider).to_string();
    let result = match kind.as_str() {
        "cursor" => cursor::fetch_local_snapshot().await,
        _ => codex::fetch_local_snapshot().await,
    };
    result.map_err(|failure| (kind, failure))
}

pub fn provider_failure_snapshot(kind: &str, failure: ProviderFailure) -> MonitorSnapshot {
    failure_snapshot(kind, failure)
}
