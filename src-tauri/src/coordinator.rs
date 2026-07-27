use crate::providers::{fetch_snapshot, provider_failure_snapshot};
use crate::snapshot::{
    assert_snapshot_safe_for_webview, cached_failure_snapshot, normalize_provider, MonitorSnapshot,
    MonitorState, ProviderErrorKind,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{Mutex, OnceCell};

const BASE_INTERVAL_SECS: u64 = 60;
const MAX_BACKOFF_SECS: u64 = 300;
const SLEEP_GAP_SECS: u64 = 90;
const BACKOFF_STEPS: [u64; 3] = [60, 120, 300];

#[derive(Default)]
struct ProviderSlot {
    inflight: Option<Arc<OnceCell<MonitorSnapshot>>>,
    fail_streak: u32,
    next_due: Option<Instant>,
}

pub struct MonitorCoordinator {
    slots: Mutex<HashMap<String, ProviderSlot>>,
    last_loop_tick: Mutex<Instant>,
    pub ui: Mutex<UiSyncState>,
    /// Test/observability counters for owner-only side effects.
    pub side_effect_count: AtomicU64,
    pub fetch_count: AtomicU64,
}

#[derive(Clone, Debug)]
pub struct UiSyncState {
    pub provider: String,
    pub language: String,
    pub view: String,
    pub always_on_top: bool,
}

impl Default for UiSyncState {
    fn default() -> Self {
        Self {
            provider: "codex".into(),
            language: "zh".into(),
            view: "dual".into(),
            always_on_top: false,
        }
    }
}

impl Default for MonitorCoordinator {
    fn default() -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
            last_loop_tick: Mutex::new(Instant::now()),
            ui: Mutex::new(UiSyncState::default()),
            side_effect_count: AtomicU64::new(0),
            fetch_count: AtomicU64::new(0),
        }
    }
}

pub fn backoff_secs_for_streak(fail_streak: u32) -> u64 {
    if fail_streak == 0 {
        return BASE_INTERVAL_SECS;
    }
    let index = (fail_streak as usize - 1).min(BACKOFF_STEPS.len() - 1);
    BACKOFF_STEPS[index].min(MAX_BACKOFF_SECS)
}

pub fn is_refresh_failure(snapshot: &MonitorSnapshot) -> bool {
    snapshot.cached
        || matches!(
            snapshot.provider.error_kind,
            Some(ProviderErrorKind::NetworkError | ProviderErrorKind::ServiceError)
        )
}

pub fn apply_backoff_after_refresh(fail_streak: u32, failed: bool, manual: bool) -> u32 {
    if failed && !manual {
        fail_streak.saturating_add(1)
    } else if !failed {
        0
    } else {
        fail_streak
    }
}

impl MonitorCoordinator {
    pub async fn refresh(
        &self,
        app: &AppHandle,
        provider: Option<&str>,
        manual: bool,
    ) -> Result<MonitorSnapshot, String> {
        let kind = normalize_provider(provider).to_string();
        if let Ok(mut last) = app.state::<MonitorState>().last_provider.lock() {
            *last = kind.clone();
        }

        let (cell, is_owner) = {
            let mut slots = self.slots.lock().await;
            let slot = slots.entry(kind.clone()).or_default();
            if let Some(existing) = &slot.inflight {
                (existing.clone(), false)
            } else {
                let cell = Arc::new(OnceCell::new());
                slot.inflight = Some(cell.clone());
                (cell, true)
            }
        };

        let snapshot = cell
            .get_or_init(|| async {
                self.fetch_count.fetch_add(1, Ordering::SeqCst);
                self.fetch_and_store(app, &kind).await
            })
            .await
            .clone();

        if is_owner {
            {
                let mut slots = self.slots.lock().await;
                if let Some(slot) = slots.get_mut(&kind) {
                    slot.inflight = None;
                    let failed = is_refresh_failure(&snapshot);
                    slot.fail_streak =
                        apply_backoff_after_refresh(slot.fail_streak, failed, manual);
                    if manual && !failed {
                        slot.fail_streak = 0;
                    }
                    let delay = backoff_secs_for_streak(slot.fail_streak);
                    slot.next_due = Some(Instant::now() + Duration::from_secs(delay));
                }
            }

            self.side_effect_count.fetch_add(1, Ordering::SeqCst);
            let _ = app.emit("monitor:snapshot", &snapshot);
            crate::tray::update_tray_from_snapshot(app, &snapshot).await;
            crate::alerts::evaluate_snapshot(app, &snapshot).await;
        }

        Ok(snapshot)
    }

    async fn fetch_and_store(&self, app: &AppHandle, kind: &str) -> MonitorSnapshot {
        let state = app.state::<MonitorState>();
        let snapshot = match fetch_snapshot(Some(kind)).await {
            Ok(snapshot) => snapshot,
            Err((kind, failure)) => {
                let cached = state
                    .snapshots
                    .lock()
                    .ok()
                    .and_then(|guard| guard.get(&kind).cloned());
                if let Some(cached) = cached {
                    cached_failure_snapshot(cached, failure)
                } else {
                    provider_failure_snapshot(&kind, failure)
                }
            }
        };
        let snapshot = match assert_snapshot_safe_for_webview(&snapshot) {
            Ok(()) => snapshot,
            Err(_) => provider_failure_snapshot(
                kind,
                crate::snapshot::provider_failure(
                    "invalid_response",
                    "Snapshot failed the WebView safety check.",
                ),
            ),
        };

        if !snapshot.cached && snapshot.provider.error_kind.is_none() {
            if let Ok(mut guard) = state.snapshots.lock() {
                guard.insert(kind.to_string(), snapshot.clone());
            }
        }
        snapshot
    }

    pub async fn tick_loop_once(&self, app: &AppHandle) {
        let now = Instant::now();
        let woke_from_sleep = {
            let mut last = self.last_loop_tick.lock().await;
            let gap = now.duration_since(*last);
            *last = now;
            gap > Duration::from_secs(SLEEP_GAP_SECS)
        };

        let active = self.ui.lock().await.provider.clone();
        let alert_providers = crate::preferences::providers_needing_background(app).await;
        let mut targets = vec![active];
        for provider in alert_providers {
            if !targets.iter().any(|item| item == &provider) {
                targets.push(provider);
            }
        }

        for kind in targets {
            let due = {
                let slots = self.slots.lock().await;
                slots
                    .get(&kind)
                    .and_then(|slot| slot.next_due)
                    .map(|due| now >= due)
                    .unwrap_or(true)
            };
            if woke_from_sleep || due {
                let _ = self.refresh(app, Some(&kind), false).await;
            }
        }
    }
}

pub fn spawn_coordinator_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            {
                let coordinator = app.state::<MonitorCoordinator>();
                coordinator.tick_loop_once(&app).await;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{ProviderAvailability, ProviderStatus, QuotaWindows};

    fn live_snapshot(kind: &str) -> MonitorSnapshot {
        MonitorSnapshot {
            provider: ProviderStatus {
                kind: kind.into(),
                source: "local".into(),
                source_label: crate::snapshot::fixed_source_label(kind).into(),
                availability: ProviderAvailability::Live,
                error_kind: None,
            },
            windows: QuotaWindows::default(),
            refreshed_at: Some("2026-07-27T00:00:00Z".into()),
            checked_at: "2026-07-27T00:00:00Z".into(),
            cached: false,
        }
    }

    #[test]
    fn backoff_steps_follow_roadmap() {
        assert_eq!(backoff_secs_for_streak(0), 60);
        assert_eq!(backoff_secs_for_streak(1), 60);
        assert_eq!(backoff_secs_for_streak(2), 120);
        assert_eq!(backoff_secs_for_streak(3), 300);
        assert_eq!(backoff_secs_for_streak(8), 300);
    }

    #[test]
    fn owner_backoff_resets_on_success_and_increments_on_failure() {
        assert_eq!(apply_backoff_after_refresh(2, false, false), 0);
        assert_eq!(apply_backoff_after_refresh(0, true, false), 1);
        assert_eq!(apply_backoff_after_refresh(1, true, true), 1);
        let failed = live_snapshot("codex");
        let mut cached = failed.clone();
        cached.cached = true;
        assert!(is_refresh_failure(&cached));
        assert!(!is_refresh_failure(&failed));
    }
}
