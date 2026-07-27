use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAvailability {
    pub available: bool,
    pub current_version: String,
    pub version: Option<String>,
    pub notes: Option<String>,
}

#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> Result<UpdateAvailability, String> {
    let current_version = app.package_info().version.to_string();
    let updater = app
        .updater_builder()
        .build()
        .map_err(|error| error.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => Ok(UpdateAvailability {
            available: true,
            current_version,
            version: Some(update.version.clone()),
            notes: update.body.clone(),
        }),
        Ok(None) => Ok(UpdateAvailability {
            available: false,
            current_version,
            version: None,
            notes: None,
        }),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
pub async fn install_app_update(app: AppHandle) -> Result<(), String> {
    let updater = app
        .updater_builder()
        .build()
        .map_err(|error| error.to_string())?;
    let Some(update) = updater.check().await.map_err(|error| error.to_string())? else {
        return Err("No update available".into());
    };
    update
        .download_and_install(|_chunk, _total| {}, || {})
        .await
        .map_err(|error| error.to_string())?;
    app.restart();
}
