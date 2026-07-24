use crate::preferences::{
    PreferencesStore, ProviderAlertRules, WindowAlertRule, WindowAlertRuntime,
};
use crate::snapshot::{MonitorSnapshot, QuotaWindow};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AlertAction {
    Threshold {
        window: &'static str,
        remaining: u32,
        threshold: u32,
    },
    Reset {
        window: &'static str,
        remaining: u32,
    },
}

pub fn evaluate_window(
    rule: &WindowAlertRule,
    runtime: &WindowAlertRuntime,
    window: Option<&QuotaWindow>,
    cached: bool,
) -> (WindowAlertRuntime, Vec<AlertAction>, &'static str) {
    let mut next = runtime.clone();
    let mut actions = Vec::new();
    if cached || !rule.enabled {
        return (next, actions, "fiveHour");
    }
    let Some(window) = window else {
        return (next, actions, "fiveHour");
    };

    let remaining = window.remaining_percent.round().clamp(0.0, 100.0) as u32;
    let cycle_key = window.resets_at.clone();

    if let (Some(previous), Some(current)) = (runtime.cycle_key.clone(), cycle_key.clone()) {
        if previous != current {
            let had_observation = runtime.last_remaining_percent.is_some();
            next.cycle_key = Some(current);
            next.notified_thresholds.clear();
            next.reset_notified = false;
            if had_observation && rule.notify_on_reset {
                actions.push(AlertAction::Reset {
                    window: "fiveHour",
                    remaining,
                });
                next.reset_notified = true;
            }
        }
    } else if runtime.cycle_key.is_none() {
        next.cycle_key = cycle_key.clone();
    }

    if cycle_key.is_none() {
        if let Some(last) = runtime.last_remaining_percent {
            for threshold in &rule.thresholds_remaining {
                if last + 5.0 <= window.remaining_percent
                    && next.notified_thresholds.contains(threshold)
                {
                    next.notified_thresholds.retain(|value| value != threshold);
                }
            }
        }
    }

    for threshold in &rule.thresholds_remaining {
        if remaining <= *threshold && !next.notified_thresholds.contains(threshold) {
            actions.push(AlertAction::Threshold {
                window: "fiveHour",
                remaining,
                threshold: *threshold,
            });
            next.notified_thresholds.push(*threshold);
        }
    }

    next.last_remaining_percent = Some(window.remaining_percent);
    (next, actions, "fiveHour")
}

fn evaluate_named_window(
    window_name: &'static str,
    rule: &WindowAlertRule,
    runtime: &WindowAlertRuntime,
    window: Option<&QuotaWindow>,
    cached: bool,
) -> (WindowAlertRuntime, Vec<AlertAction>) {
    let (mut next, mut actions, _) = evaluate_window(rule, runtime, window, cached);
    for action in &mut actions {
        match action {
            AlertAction::Threshold { window, .. } | AlertAction::Reset { window, .. } => {
                *window = window_name;
            }
        }
    }
    // Fix reset_notified path for seven_day naming already handled via rewrite
    if cached || !rule.enabled || window.is_none() {
        return (next, actions);
    }
    next.last_remaining_percent = window.map(|item| item.remaining_percent);
    (next, actions)
}

pub fn evaluate_provider(
    rules: &ProviderAlertRules,
    runtime: &mut crate::preferences::ProviderAlertRuntime,
    snapshot: &MonitorSnapshot,
) -> Vec<AlertAction> {
    let (five_runtime, mut actions) = evaluate_named_window(
        "fiveHour",
        &rules.five_hour,
        &runtime.five_hour,
        snapshot.windows.five_hour.as_ref(),
        snapshot.cached,
    );
    runtime.five_hour = five_runtime;

    let (seven_runtime, seven_actions) = evaluate_named_window(
        "sevenDay",
        &rules.seven_day,
        &runtime.seven_day,
        snapshot.windows.seven_day.as_ref(),
        snapshot.cached,
    );
    runtime.seven_day = seven_runtime;
    actions.extend(seven_actions);
    actions
}

pub async fn ensure_notification_permission(app: &AppHandle) -> Result<bool, String> {
    let permission = app
        .notification()
        .permission_state()
        .map_err(|error| error.to_string())?;
    match permission {
        tauri_plugin_notification::PermissionState::Granted => Ok(true),
        tauri_plugin_notification::PermissionState::Denied => Ok(false),
        _ => {
            let requested = app
                .notification()
                .request_permission()
                .map_err(|error| error.to_string())?;
            Ok(matches!(
                requested,
                tauri_plugin_notification::PermissionState::Granted
            ))
        }
    }
}

pub async fn evaluate_snapshot(app: &AppHandle, snapshot: &MonitorSnapshot) {
    let Some(store) = app.try_state::<PreferencesStore>() else {
        return;
    };
    let prefs = store.get_preferences().await;
    if !prefs.enabled || prefs.notification_denied {
        return;
    }
    let provider = snapshot.provider.kind.as_str();
    let rules = store.provider_rules(provider).await;
    let mut runtime = store.get_runtime().await;
    let provider_runtime = if provider == "cursor" {
        &mut runtime.cursor
    } else {
        &mut runtime.codex
    };
    let actions = evaluate_provider(&rules, provider_runtime, snapshot);
    let _ = store.set_runtime(runtime).await;

    let language = app
        .state::<crate::coordinator::MonitorCoordinator>()
        .ui
        .lock()
        .await
        .language
        .clone();

    for action in actions {
        let (title, body) = notification_copy(provider, &action, &language);
        let _ = app.notification().builder().title(title).body(body).show();
    }
}

fn notification_copy(provider: &str, action: &AlertAction, language: &str) -> (String, String) {
    let provider_label = if provider == "cursor" {
        "Cursor"
    } else {
        "Codex"
    };
    let window_label = match action {
        AlertAction::Threshold { window, .. } | AlertAction::Reset { window, .. } => {
            window_label(provider, window, language)
        }
    };
    match action {
        AlertAction::Threshold {
            remaining,
            threshold,
            ..
        } => {
            if language == "en" {
                (
                    format!("{provider_label} quota alert"),
                    format!("{window_label} remaining {remaining}% (≤{threshold}%)"),
                )
            } else {
                (
                    format!("{provider_label} 额度提醒"),
                    format!("{window_label} 剩余 {remaining}%（≤{threshold}%）"),
                )
            }
        }
        AlertAction::Reset { remaining, .. } => {
            if language == "en" {
                (
                    format!("{provider_label} quota reset"),
                    format!("{window_label} reset · remaining {remaining}%"),
                )
            } else {
                (
                    format!("{provider_label} 额度已重置"),
                    format!("{window_label} 已重置 · 剩余 {remaining}%"),
                )
            }
        }
    }
}

fn window_label(provider: &str, window: &str, language: &str) -> String {
    match (provider, window, language) {
        ("cursor", "fiveHour", "en") => "First-party".into(),
        ("cursor", "sevenDay", "en") => "API".into(),
        ("cursor", "fiveHour", _) => "订阅额度".into(),
        ("cursor", "sevenDay", _) => "API 额度".into(),
        (_, "sevenDay", "en") => "7-day".into(),
        (_, "sevenDay", _) => "7 天".into(),
        (_, _, "en") => "5-hour".into(),
        _ => "5 小时".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{AccountSummary, ProviderAvailability, ProviderStatus, QuotaWindows};

    fn snapshot(remaining: f64, resets_at: Option<&str>, cached: bool) -> MonitorSnapshot {
        MonitorSnapshot {
            account: AccountSummary {
                display_name: "demo".into(),
                plan: "plus".into(),
            },
            provider: ProviderStatus {
                kind: "codex".into(),
                source: "local".into(),
                auth_path_label: "~/.codex/auth.json".into(),
                availability: ProviderAvailability::Live,
                error_kind: None,
            },
            windows: QuotaWindows {
                five_hour: Some(QuotaWindow {
                    used_percent: 100.0 - remaining,
                    remaining_percent: remaining,
                    duration_seconds: 18_000,
                    reset_after_seconds: Some(60),
                    resets_at: resets_at.map(str::to_string),
                    unit: None,
                    used_amount: None,
                    limit_amount: None,
                }),
                seven_day: None,
            },
            refreshed_at: Some("2026-07-24T00:00:00Z".into()),
            checked_at: "2026-07-24T00:00:00Z".into(),
            cached,
        }
    }

    #[test]
    fn notifies_threshold_once_per_cycle() {
        let rules = ProviderAlertRules {
            five_hour: WindowAlertRule {
                enabled: true,
                thresholds_remaining: vec![10],
                notify_on_reset: true,
            },
            ..ProviderAlertRules::default()
        };
        let mut runtime = crate::preferences::ProviderAlertRuntime::default();
        let first = evaluate_provider(&rules, &mut runtime, &snapshot(9.0, Some("a"), false));
        assert_eq!(first.len(), 1);
        let second = evaluate_provider(&rules, &mut runtime, &snapshot(8.0, Some("a"), false));
        assert!(second.is_empty());
    }

    #[test]
    fn reset_cycle_clears_thresholds_and_can_notify_reset() {
        let rules = ProviderAlertRules {
            five_hour: WindowAlertRule {
                enabled: true,
                thresholds_remaining: vec![10],
                notify_on_reset: true,
            },
            ..ProviderAlertRules::default()
        };
        let mut runtime = crate::preferences::ProviderAlertRuntime::default();
        let _ = evaluate_provider(&rules, &mut runtime, &snapshot(9.0, Some("a"), false));
        let actions = evaluate_provider(&rules, &mut runtime, &snapshot(95.0, Some("b"), false));
        assert!(actions
            .iter()
            .any(|action| matches!(action, AlertAction::Reset { .. })));
        assert!(runtime.five_hour.notified_thresholds.is_empty());
    }

    #[test]
    fn without_resets_at_rearms_after_five_point_recovery() {
        let rule = WindowAlertRule {
            enabled: true,
            thresholds_remaining: vec![10],
            notify_on_reset: true,
        };
        let mut runtime = WindowAlertRuntime {
            notified_thresholds: vec![10],
            last_remaining_percent: Some(8.0),
            ..WindowAlertRuntime::default()
        };
        let window = QuotaWindow {
            used_percent: 84.0,
            remaining_percent: 16.0,
            duration_seconds: 18_000,
            reset_after_seconds: None,
            resets_at: None,
            unit: None,
            used_amount: None,
            limit_amount: None,
        };
        let (next, actions, _) = evaluate_window(&rule, &runtime, Some(&window), false);
        assert!(!next.notified_thresholds.contains(&10));
        assert!(actions.is_empty());
        runtime = next;
        let window = QuotaWindow {
            remaining_percent: 9.0,
            used_percent: 91.0,
            ..window
        };
        let (_, actions, _) = evaluate_window(&rule, &runtime, Some(&window), false);
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn cached_snapshots_do_not_notify() {
        let rules = ProviderAlertRules::default();
        let mut runtime = crate::preferences::ProviderAlertRuntime::default();
        let actions = evaluate_provider(&rules, &mut runtime, &snapshot(5.0, Some("a"), true));
        assert!(actions.is_empty());
    }
}
