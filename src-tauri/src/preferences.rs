use crate::snapshot::normalize_provider;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowAlertRule {
    pub enabled: bool,
    pub thresholds_remaining: Vec<u32>,
    pub notify_on_reset: bool,
}

impl Default for WindowAlertRule {
    fn default() -> Self {
        Self {
            enabled: true,
            thresholds_remaining: vec![10],
            notify_on_reset: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAlertRules {
    pub five_hour: WindowAlertRule,
    pub seven_day: WindowAlertRule,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MonitoringPreferences {
    pub enabled: bool,
    pub notification_denied: bool,
    pub codex: ProviderAlertRules,
    pub cursor: ProviderAlertRules,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct WindowAlertRuntime {
    pub cycle_key: Option<String>,
    pub notified_thresholds: Vec<u32>,
    pub reset_notified: bool,
    pub last_remaining_percent: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAlertRuntime {
    pub five_hour: WindowAlertRuntime,
    pub seven_day: WindowAlertRuntime,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AlertRuntimeState {
    pub codex: ProviderAlertRuntime,
    pub cursor: ProviderAlertRuntime,
}

pub struct PreferencesStore {
    prefs: Mutex<MonitoringPreferences>,
    runtime: Mutex<AlertRuntimeState>,
    config_dir: PathBuf,
}

impl PreferencesStore {
    pub fn load(app: &AppHandle) -> Result<Self, String> {
        let config_dir = app
            .path()
            .app_config_dir()
            .map_err(|error| error.to_string())?;
        fs::create_dir_all(&config_dir).map_err(|error| error.to_string())?;
        let prefs = read_or_default::<MonitoringPreferences>(&config_dir.join("monitoring.json"));
        let runtime = read_or_default::<AlertRuntimeState>(&config_dir.join("alert-runtime.json"));
        Ok(Self {
            prefs: Mutex::new(prefs),
            runtime: Mutex::new(runtime),
            config_dir,
        })
    }

    pub async fn get_preferences(&self) -> MonitoringPreferences {
        self.prefs.lock().await.clone()
    }

    pub async fn set_preferences(
        &self,
        next: MonitoringPreferences,
    ) -> Result<MonitoringPreferences, String> {
        atomic_write(
            &self.config_dir.join("monitoring.json"),
            &serde_json::to_vec_pretty(&next).map_err(|error| error.to_string())?,
        )?;
        *self.prefs.lock().await = next.clone();
        Ok(next)
    }

    pub async fn get_runtime(&self) -> AlertRuntimeState {
        self.runtime.lock().await.clone()
    }

    pub async fn set_runtime(&self, next: AlertRuntimeState) -> Result<(), String> {
        atomic_write(
            &self.config_dir.join("alert-runtime.json"),
            &serde_json::to_vec_pretty(&next).map_err(|error| error.to_string())?,
        )?;
        *self.runtime.lock().await = next;
        Ok(())
    }

    pub async fn provider_rules(&self, provider: &str) -> ProviderAlertRules {
        let prefs = self.prefs.lock().await;
        if normalize_provider(Some(provider)) == "cursor" {
            prefs.cursor.clone()
        } else {
            prefs.codex.clone()
        }
    }
}

fn read_or_default<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> T {
    match fs::read_to_string(path) {
        Ok(raw) => match serde_json::from_str(&raw) {
            Ok(value) => value,
            Err(_) => {
                let broken = path.with_extension("json.broken");
                let _ = fs::rename(path, broken);
                T::default()
            }
        },
        Err(_) => T::default(),
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("tmp");
    fs::write(&temp, bytes).map_err(|error| error.to_string())?;
    fs::rename(&temp, path).map_err(|error| error.to_string())
}

pub async fn providers_needing_background(app: &AppHandle) -> Vec<String> {
    let Some(store) = app.try_state::<PreferencesStore>() else {
        return Vec::new();
    };
    let prefs = store.get_preferences().await;
    if !prefs.enabled {
        return Vec::new();
    }
    let mut providers = Vec::new();
    if prefs.codex.five_hour.enabled || prefs.codex.seven_day.enabled {
        providers.push("codex".into());
    }
    if prefs.cursor.five_hour.enabled || prefs.cursor.seven_day.enabled {
        providers.push("cursor".into());
    }
    providers
}

#[tauri::command]
pub async fn get_monitor_preferences(
    store: tauri::State<'_, PreferencesStore>,
) -> Result<MonitoringPreferences, String> {
    Ok(store.get_preferences().await)
}

#[tauri::command]
pub async fn set_monitor_preferences(
    app: AppHandle,
    store: tauri::State<'_, PreferencesStore>,
    preferences: MonitoringPreferences,
) -> Result<MonitoringPreferences, String> {
    let mut next = preferences;
    if next.enabled {
        match crate::alerts::ensure_notification_permission(&app).await {
            Ok(true) => next.notification_denied = false,
            Ok(false) => {
                next.enabled = false;
                next.notification_denied = true;
            }
            Err(error) => return Err(error),
        }
    }
    store.set_preferences(next).await
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiPreferencesPayload {
    pub provider: Option<String>,
    pub language: Option<String>,
    pub view: Option<String>,
    pub always_on_top: Option<bool>,
}

#[tauri::command]
pub async fn sync_ui_preferences(
    app: AppHandle,
    payload: UiPreferencesPayload,
) -> Result<(), String> {
    let coordinator = app.state::<crate::coordinator::MonitorCoordinator>();
    let mut ui = coordinator.ui.lock().await;
    if let Some(provider) = payload.provider {
        ui.provider = normalize_provider(Some(&provider)).to_string();
    }
    if let Some(language) = payload.language {
        ui.language = if language == "en" { "en" } else { "zh" }.into();
    }
    if let Some(view) = payload.view {
        ui.view = if view == "focus" { "focus" } else { "dual" }.into();
    }
    if let Some(always_on_top) = payload.always_on_top {
        ui.always_on_top = always_on_top;
    }
    drop(ui);
    crate::tray::rebuild_tray_menu(&app).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn damaged_config_falls_back_and_keeps_broken_file() {
        let dir = env::temp_dir().join(format!("token-monitor-prefs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("monitoring.json");
        fs::write(&path, "{not-json").unwrap();
        let value = read_or_default::<MonitoringPreferences>(&path);
        assert_eq!(value, MonitoringPreferences::default());
        assert!(dir.join("monitoring.json.broken").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
