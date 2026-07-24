use crate::providers::{fetch_snapshot, provider_failure_snapshot};
use crate::snapshot::{cached_failure_snapshot, normalize_provider, MonitorSnapshot, MonitorState};
use std::collections::HashMap;
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

        let cell = {
            let mut slots = self.slots.lock().await;
            let slot = slots.entry(kind.clone()).or_default();
            if let Some(existing) = &slot.inflight {
                existing.clone()
            } else {
                let cell = Arc::new(OnceCell::new());
                slot.inflight = Some(cell.clone());
                cell
            }
        };

        let snapshot = cell
            .get_or_init(|| async { self.fetch_and_store(app, &kind).await })
            .await
            .clone();

        {
            let mut slots = self.slots.lock().await;
            if let Some(slot) = slots.get_mut(&kind) {
                slot.inflight = None;
                let failed = snapshot.cached
                    || matches!(
                        snapshot.provider.error_kind,
                        Some(
                            crate::snapshot::ProviderErrorKind::NetworkError
                                | crate::snapshot::ProviderErrorKind::ServiceError
                        )
                    );
                if failed && !manual {
                    slot.fail_streak = slot.fail_streak.saturating_add(1);
                } else if !snapshot.cached && snapshot.provider.error_kind.is_none() {
                    slot.fail_streak = 0;
                }
                if manual && !snapshot.cached && snapshot.provider.error_kind.is_none() {
                    slot.fail_streak = 0;
                }
                let delay = backoff_secs_for_streak(slot.fail_streak);
                slot.next_due = Some(Instant::now() + Duration::from_secs(delay));
            }
        }

        let _ = app.emit("monitor:snapshot", &snapshot);
        crate::tray::update_tray_from_snapshot(app, &snapshot).await;
        crate::alerts::evaluate_snapshot(app, &snapshot).await;
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

    #[test]
    fn backoff_steps_follow_roadmap() {
        assert_eq!(backoff_secs_for_streak(0), 60);
        assert_eq!(backoff_secs_for_streak(1), 60);
        assert_eq!(backoff_secs_for_streak(2), 120);
        assert_eq!(backoff_secs_for_streak(3), 300);
        assert_eq!(backoff_secs_for_streak(8), 300);
    }
}
