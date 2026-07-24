#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod alerts;
mod autostart;
mod coordinator;
mod lifecycle;
mod preferences;
mod providers;
mod snapshot;
mod tray;
mod updater;

use autostart::{initialize_start_at_login, set_start_at_login};
use coordinator::{spawn_coordinator_loop, MonitorCoordinator};
use lifecycle::LifecycleState;
use preferences::{
    get_monitor_preferences, set_monitor_preferences, sync_ui_preferences, PreferencesStore,
};
use snapshot::{MonitorSnapshot, MonitorState};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use updater::{check_app_update, install_app_update};

#[tauri::command]
async fn refresh_monitor_data(
    app: AppHandle,
    coordinator: tauri::State<'_, MonitorCoordinator>,
    provider: Option<String>,
) -> Result<MonitorSnapshot, String> {
    coordinator.refresh(&app, provider.as_deref(), true).await
}

#[tauri::command]
async fn get_monitor_snapshot(
    app: AppHandle,
    coordinator: tauri::State<'_, MonitorCoordinator>,
    provider: Option<String>,
) -> Result<MonitorSnapshot, String> {
    coordinator.refresh(&app, provider.as_deref(), true).await
}

#[tauri::command]
fn start_window_drag(window: tauri::WebviewWindow) -> Result<(), String> {
    window.start_dragging().map_err(|error| error.to_string())
}

#[tauri::command]
fn set_always_on_top(window: tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    window
        .set_always_on_top(enabled)
        .map_err(|error| error.to_string())
}

fn main() {
    let autostart = tauri_plugin_autostart::Builder::new().app_name(autostart::NEW_APP_NAME);
    #[cfg(target_os = "macos")]
    let autostart = autostart.macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent);

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            lifecycle::show_main_window(app);
        }))
        .manage(LifecycleState::default())
        .manage(MonitorState::default())
        .manage(MonitorCoordinator::default())
        .plugin(autostart.build())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let store = PreferencesStore::load(app.handle())?;
            app.manage(store);
            tray::setup_tray(app)?;
            spawn_coordinator_loop(app.handle().clone());
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tray::rebuild_tray_menu(&handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_monitor_snapshot,
            refresh_monitor_data,
            start_window_drag,
            set_always_on_top,
            initialize_start_at_login,
            set_start_at_login,
            get_monitor_preferences,
            set_monitor_preferences,
            sync_ui_preferences,
            check_app_update,
            install_app_update
        ])
        .build(tauri::generate_context!())
        .expect("error while building Token Monitor")
        .run(|app, event| match event {
            RunEvent::ExitRequested { .. } => {
                app.state::<LifecycleState>().request_exit();
            }
            RunEvent::WindowEvent {
                label,
                event: WindowEvent::CloseRequested { api, .. },
                ..
            } if label == "main" && !app.state::<LifecycleState>().is_exiting() => {
                api.prevent_close();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            _ => {}
        });
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
            reset_at: Some(reset_at),
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
        let mapped = quota_window(window(120.0, FIVE_HOURS_SECONDS, 1_800_000_000));
        assert_eq!(mapped.used_percent, 100.0);
        assert_eq!(mapped.remaining_percent, 0.0);
    }

    #[test]
    fn preserves_codex_window_when_reset_is_missing() {
        let mut response = window(25.0, FIVE_HOURS_SECONDS, 1_800_000_000);
        response.reset_at = None;
        let mapped = quota_window(response);
        assert!(mapped.resets_at.is_none());
    }
}
