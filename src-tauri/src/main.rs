#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod providers;
mod snapshot;

use chrono::Utc;
use providers::{fetch_snapshot, provider_failure_snapshot};
use snapshot::{normalize_provider, MonitorSnapshot, MonitorState};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewWindow,
};
use tauri_plugin_autostart::ManagerExt;

fn update_tray_tooltip(app: &AppHandle, snapshot: &MonitorSnapshot) {
    let kind = snapshot.provider.kind.as_str();
    let (primary_label, secondary_label) = match kind {
        "cursor" => ("Included", "On-demand"),
        _ => ("5h", "7d"),
    };
    let primary = snapshot
        .windows
        .five_hour
        .as_ref()
        .map(|window| format!("{primary_label} {:.0}%", window.remaining_percent))
        .unwrap_or_else(|| format!("{primary_label} —"));
    let secondary = snapshot
        .windows
        .seven_day
        .as_ref()
        .map(|window| format!("{secondary_label} {:.0}%", window.remaining_percent))
        .unwrap_or_else(|| format!("{secondary_label} —"));
    let title = match kind {
        "cursor" => "Cursor Usage Monitor",
        _ => "Codex Usage Monitor",
    };
    if let Some(tray) = app.tray_by_id("monitor-tray") {
        let _ = tray.set_tooltip(Some(format!("{title} · {primary} · {secondary}")));
    }
}

#[tauri::command]
async fn refresh_monitor_data(
    app: AppHandle,
    state: tauri::State<'_, MonitorState>,
    provider: Option<String>,
) -> Result<MonitorSnapshot, String> {
    let kind = normalize_provider(provider.as_deref()).to_string();
    if let Ok(mut last) = state.last_provider.lock() {
        *last = kind.clone();
    }

    let snapshot = match fetch_snapshot(Some(&kind)).await {
        Ok(snapshot) => snapshot,
        Err((kind, failure)) => {
            let cached = state
                .snapshots
                .lock()
                .map_err(|_| "Snapshot cache is unavailable".to_string())?
                .get(&kind)
                .cloned();
            if let Some(mut cached) = cached {
                cached.provider.connected = false;
                cached.provider.state = "stale".into();
                cached.provider.message =
                    format!("{} Showing the last successful snapshot.", failure.message);
                cached.checked_at = Utc::now().to_rfc3339();
                cached.cached = true;
                cached
            } else {
                provider_failure_snapshot(&kind, failure)
            }
        }
    };

    if !snapshot.cached && snapshot.provider.connected {
        state
            .snapshots
            .lock()
            .map_err(|_| "Snapshot cache is unavailable".to_string())?
            .insert(kind, snapshot.clone());
    }
    update_tray_tooltip(&app, &snapshot);
    Ok(snapshot)
}

#[tauri::command]
async fn get_monitor_snapshot(
    app: AppHandle,
    state: tauri::State<'_, MonitorState>,
    provider: Option<String>,
) -> Result<MonitorSnapshot, String> {
    refresh_monitor_data(app, state, provider).await
}

#[tauri::command]
fn start_window_drag(window: WebviewWindow) -> Result<(), String> {
    window.start_dragging().map_err(|error| error.to_string())
}

#[tauri::command]
fn set_always_on_top(window: WebviewWindow, enabled: bool) -> Result<(), String> {
    window
        .set_always_on_top(enabled)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_start_at_login(app: AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|error| error.to_string())
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Open Usage Monitor", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh usage", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &refresh, &quit])?;
    TrayIconBuilder::with_id("monitor-tray")
        .tooltip("Usage Monitor")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "refresh" => {
                let _ = app.emit("monitor:refresh", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(event, tauri::tray::TrayIconEvent::DoubleClick { .. }) {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .manage(MonitorState::default())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            setup_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_monitor_snapshot,
            refresh_monitor_data,
            start_window_drag,
            set_always_on_top,
            set_start_at_login
        ])
        .run(tauri::generate_context!())
        .expect("error while running Usage Monitor");
}

#[cfg(test)]
mod tests {
    use crate::providers::codex::{
        classify_windows, quota_window, RateLimitDetails, UsageWindowResponse, FIVE_HOURS_SECONDS,
        SEVEN_DAYS_SECONDS,
    };

    fn window(used: f64, duration: i64, reset_at: i64) -> UsageWindowResponse {
        UsageWindowResponse {
            used_percent: used,
            limit_window_seconds: duration,
            reset_after_seconds: Some(10),
            reset_at,
        }
    }

    #[test]
    fn maps_windows_by_duration_not_api_position() {
        let rate_limit = RateLimitDetails {
            primary_window: Some(window(41.0, SEVEN_DAYS_SECONDS, 1_800_000_000)),
            secondary_window: Some(window(25.0, FIVE_HOURS_SECONDS, 1_800_000_100)),
        };
        let mapped = classify_windows(Some(rate_limit));
        assert_eq!(mapped.five_hour.unwrap().remaining_percent, 75.0);
        assert_eq!(mapped.seven_day.unwrap().remaining_percent, 59.0);
    }

    #[test]
    fn leaves_unknown_or_missing_windows_unavailable() {
        let rate_limit = RateLimitDetails {
            primary_window: Some(window(10.0, 3_600, 1_800_000_000)),
            secondary_window: None,
        };
        let mapped = classify_windows(Some(rate_limit));
        assert!(mapped.five_hour.is_none());
        assert!(mapped.seven_day.is_none());
    }

    #[test]
    fn clamps_invalid_percentages() {
        let mapped = quota_window(window(120.0, FIVE_HOURS_SECONDS, 1_800_000_000)).unwrap();
        assert_eq!(mapped.used_percent, 100.0);
        assert_eq!(mapped.remaining_percent, 0.0);
    }
}
