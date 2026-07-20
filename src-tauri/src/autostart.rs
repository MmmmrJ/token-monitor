use auto_launch::AutoLaunch;
use serde::Serialize;
use std::env::current_exe;
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

pub const NEW_APP_NAME: &str = "Token Monitor";
pub const OLD_APP_NAME: &str = "Codex Usage Monitor";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
pub enum AutostartErrorKind {
    QueryFailed,
    EnableFailed,
    CleanupFailed,
    DisableFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutostartStatus {
    pub enabled: bool,
    pub migrated: bool,
    pub error_kind: Option<AutostartErrorKind>,
}

impl AutostartStatus {
    fn ok(enabled: bool, migrated: bool) -> Self {
        Self {
            enabled,
            migrated,
            error_kind: None,
        }
    }

    fn err(enabled: bool, error_kind: AutostartErrorKind) -> Self {
        Self {
            enabled,
            migrated: false,
            error_kind: Some(error_kind),
        }
    }
}

/// Platform-backed store operations used by migration decisions.
pub trait AutostartStore {
    fn is_new_enabled(&self) -> Result<bool, ()>;
    fn is_old_enabled(&self) -> Result<bool, ()>;
    fn enable_new(&mut self) -> Result<(), ()>;
    fn disable_new(&mut self) -> Result<(), ()>;
    fn disable_old(&mut self) -> Result<(), ()>;
}

fn probe(store: &impl AutostartStore) -> Result<(bool, bool), AutostartErrorKind> {
    let new_enabled = store
        .is_new_enabled()
        .map_err(|_| AutostartErrorKind::QueryFailed)?;
    let old_enabled = store
        .is_old_enabled()
        .map_err(|_| AutostartErrorKind::QueryFailed)?;
    Ok((new_enabled, old_enabled))
}

fn ensure_new_enabled(
    store: &mut impl AutostartStore,
    old_enabled: bool,
) -> Result<bool, AutostartStatus> {
    if store.enable_new().is_err() {
        return Err(AutostartStatus::err(
            old_enabled,
            AutostartErrorKind::EnableFailed,
        ));
    }
    match store.is_new_enabled() {
        Ok(true) => Ok(true),
        Ok(false) | Err(()) => Err(AutostartStatus::err(
            old_enabled,
            AutostartErrorKind::EnableFailed,
        )),
    }
}

fn cleanup_old_if_present(
    store: &mut impl AutostartStore,
    old_enabled: bool,
    enabled: bool,
) -> Result<bool, AutostartStatus> {
    if !old_enabled {
        return Ok(false);
    }
    if store.disable_old().is_err() {
        return Err(AutostartStatus::err(
            enabled,
            AutostartErrorKind::CleanupFailed,
        ));
    }
    match store.is_old_enabled() {
        Ok(false) => Ok(true),
        Ok(true) | Err(()) => Err(AutostartStatus::err(
            enabled,
            AutostartErrorKind::CleanupFailed,
        )),
    }
}

/// Initialize / migrate login items. Target is enabled when the new item, old item,
/// or saved preference is enabled. New item is created and verified before old cleanup.
pub fn initialize_autostart(
    store: &mut impl AutostartStore,
    preferred_enabled: bool,
) -> AutostartStatus {
    let (new_enabled, old_enabled) = match probe(store) {
        Ok(values) => values,
        Err(kind) => return AutostartStatus::err(false, kind),
    };

    let target_enabled = new_enabled || old_enabled || preferred_enabled;
    if !target_enabled {
        return AutostartStatus::ok(false, false);
    }

    let enabled = if new_enabled {
        true
    } else {
        match ensure_new_enabled(store, old_enabled) {
            Ok(value) => value,
            Err(status) => return status,
        }
    };

    match cleanup_old_if_present(store, old_enabled, enabled) {
        Ok(migrated) => AutostartStatus::ok(enabled, migrated),
        Err(status) => status,
    }
}

/// User-driven toggle. Enable creates/verifies the new item then cleans the old one.
/// Disable clears both entries and reports the effective system state.
pub fn set_autostart(store: &mut impl AutostartStore, enabled: bool) -> AutostartStatus {
    if enabled {
        let old_enabled = match store.is_old_enabled() {
            Ok(value) => value,
            Err(()) => return AutostartStatus::err(false, AutostartErrorKind::QueryFailed),
        };
        let new_enabled = match store.is_new_enabled() {
            Ok(value) => value,
            Err(()) => return AutostartStatus::err(false, AutostartErrorKind::QueryFailed),
        };

        let enabled = if new_enabled {
            true
        } else {
            match ensure_new_enabled(store, old_enabled) {
                Ok(value) => value,
                Err(status) => return status,
            }
        };

        return match cleanup_old_if_present(store, old_enabled, enabled) {
            Ok(migrated) => AutostartStatus::ok(enabled, migrated),
            Err(status) => status,
        };
    }

    let mut disable_failed = store.disable_new().is_err();
    if store.disable_old().is_err() {
        disable_failed = true;
    }

    let (new_enabled, old_enabled) = match probe(store) {
        Ok(values) => values,
        Err(kind) => return AutostartStatus::err(true, kind),
    };
    let still_enabled = new_enabled || old_enabled;
    if disable_failed || still_enabled {
        AutostartStatus::err(still_enabled, AutostartErrorKind::DisableFailed)
    } else {
        AutostartStatus::ok(false, false)
    }
}

fn resolve_app_path() -> Result<String, String> {
    let current_exe = current_exe().map_err(|error| error.to_string())?;
    #[cfg(target_os = "macos")]
    {
        // Match tauri-plugin-autostart LaunchAgent path (executable inside .app).
        Ok(current_exe
            .canonicalize()
            .map_err(|error| error.to_string())?
            .display()
            .to_string())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(current_exe.display().to_string())
    }
}

fn build_old_auto_launch() -> Result<AutoLaunch, String> {
    let app_path = resolve_app_path()?;
    #[cfg(target_os = "macos")]
    {
        Ok(AutoLaunch::new(
            OLD_APP_NAME,
            &app_path,
            true,
            &[] as &[&str],
        ))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(AutoLaunch::new(OLD_APP_NAME, &app_path, &[] as &[&str]))
    }
}

struct SystemAutostart<'a> {
    app: &'a AppHandle,
    old: AutoLaunch,
}

impl AutostartStore for SystemAutostart<'_> {
    fn is_new_enabled(&self) -> Result<bool, ()> {
        self.app.autolaunch().is_enabled().map_err(|_| ())
    }

    fn is_old_enabled(&self) -> Result<bool, ()> {
        self.old.is_enabled().map_err(|_| ())
    }

    fn enable_new(&mut self) -> Result<(), ()> {
        self.app.autolaunch().enable().map_err(|_| ())
    }

    fn disable_new(&mut self) -> Result<(), ()> {
        self.app.autolaunch().disable().map_err(|_| ())
    }

    fn disable_old(&mut self) -> Result<(), ()> {
        self.old.disable().map_err(|_| ())
    }
}

fn with_system_store(
    app: &AppHandle,
    run: impl FnOnce(&mut SystemAutostart<'_>) -> AutostartStatus,
) -> AutostartStatus {
    let old = match build_old_auto_launch() {
        Ok(value) => value,
        Err(_) => return AutostartStatus::err(false, AutostartErrorKind::QueryFailed),
    };
    let mut store = SystemAutostart { app, old };
    run(&mut store)
}

#[tauri::command]
pub fn initialize_start_at_login(app: AppHandle, preferred_enabled: bool) -> AutostartStatus {
    with_system_store(&app, |store| initialize_autostart(store, preferred_enabled))
}

#[tauri::command]
pub fn set_start_at_login(app: AppHandle, enabled: bool) -> AutostartStatus {
    with_system_store(&app, |store| set_autostart(store, enabled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockStore {
        new_enabled: bool,
        old_enabled: bool,
        fail_query_new: bool,
        fail_query_old: bool,
        fail_enable_new: bool,
        fail_disable_new: bool,
        fail_disable_old: bool,
        enable_new_calls: usize,
        disable_new_calls: usize,
        disable_old_calls: usize,
    }

    impl AutostartStore for MockStore {
        fn is_new_enabled(&self) -> Result<bool, ()> {
            if self.fail_query_new {
                Err(())
            } else {
                Ok(self.new_enabled)
            }
        }

        fn is_old_enabled(&self) -> Result<bool, ()> {
            if self.fail_query_old {
                Err(())
            } else {
                Ok(self.old_enabled)
            }
        }

        fn enable_new(&mut self) -> Result<(), ()> {
            self.enable_new_calls += 1;
            if self.fail_enable_new {
                return Err(());
            }
            self.new_enabled = true;
            Ok(())
        }

        fn disable_new(&mut self) -> Result<(), ()> {
            self.disable_new_calls += 1;
            if self.fail_disable_new {
                return Err(());
            }
            self.new_enabled = false;
            Ok(())
        }

        fn disable_old(&mut self) -> Result<(), ()> {
            self.disable_old_calls += 1;
            if self.fail_disable_old {
                return Err(());
            }
            self.old_enabled = false;
            Ok(())
        }
    }

    #[test]
    fn both_absent_preferred_off_stays_off() {
        let mut store = MockStore::default();
        let status = initialize_autostart(&mut store, false);
        assert_eq!(status, AutostartStatus::ok(false, false));
        assert_eq!(store.enable_new_calls, 0);
        assert_eq!(store.disable_old_calls, 0);
    }

    #[test]
    fn both_absent_preferred_on_creates_new() {
        let mut store = MockStore::default();
        let status = initialize_autostart(&mut store, true);
        assert_eq!(status, AutostartStatus::ok(true, false));
        assert!(store.new_enabled);
        assert_eq!(store.enable_new_calls, 1);
        assert_eq!(store.disable_old_calls, 0);
    }

    #[test]
    fn only_old_enabled_migrates_to_new() {
        let mut store = MockStore {
            old_enabled: true,
            ..MockStore::default()
        };
        let status = initialize_autostart(&mut store, false);
        assert_eq!(status, AutostartStatus::ok(true, true));
        assert!(store.new_enabled);
        assert!(!store.old_enabled);
        assert_eq!(store.enable_new_calls, 1);
        assert_eq!(store.disable_old_calls, 1);
    }

    #[test]
    fn only_new_enabled_keeps_enabled_without_extra_work() {
        let mut store = MockStore {
            new_enabled: true,
            ..MockStore::default()
        };
        let status = initialize_autostart(&mut store, false);
        assert_eq!(status, AutostartStatus::ok(true, false));
        assert_eq!(store.enable_new_calls, 0);
        assert_eq!(store.disable_old_calls, 0);
    }

    #[test]
    fn both_enabled_keeps_new_and_cleans_old() {
        let mut store = MockStore {
            new_enabled: true,
            old_enabled: true,
            ..MockStore::default()
        };
        let status = initialize_autostart(&mut store, false);
        assert_eq!(status, AutostartStatus::ok(true, true));
        assert!(store.new_enabled);
        assert!(!store.old_enabled);
        assert_eq!(store.enable_new_calls, 0);
        assert_eq!(store.disable_old_calls, 1);
    }

    #[test]
    fn enable_failure_preserves_old() {
        let mut store = MockStore {
            old_enabled: true,
            fail_enable_new: true,
            ..MockStore::default()
        };
        let status = initialize_autostart(&mut store, true);
        assert_eq!(
            status,
            AutostartStatus::err(true, AutostartErrorKind::EnableFailed)
        );
        assert!(!store.new_enabled);
        assert!(store.old_enabled);
        assert_eq!(store.disable_old_calls, 0);
    }

    #[test]
    fn cleanup_failure_keeps_enabled_with_error() {
        let mut store = MockStore {
            old_enabled: true,
            fail_disable_old: true,
            ..MockStore::default()
        };
        let status = initialize_autostart(&mut store, false);
        assert_eq!(
            status,
            AutostartStatus::err(true, AutostartErrorKind::CleanupFailed)
        );
        assert!(store.new_enabled);
        assert!(store.old_enabled);
    }

    #[test]
    fn disable_clears_both_entries() {
        let mut store = MockStore {
            new_enabled: true,
            old_enabled: true,
            ..MockStore::default()
        };
        let status = set_autostart(&mut store, false);
        assert_eq!(status, AutostartStatus::ok(false, false));
        assert!(!store.new_enabled);
        assert!(!store.old_enabled);
        assert_eq!(store.disable_new_calls, 1);
        assert_eq!(store.disable_old_calls, 1);
    }

    #[test]
    fn migration_is_idempotent_on_second_run() {
        let mut store = MockStore {
            old_enabled: true,
            ..MockStore::default()
        };
        let first = initialize_autostart(&mut store, false);
        assert_eq!(first, AutostartStatus::ok(true, true));

        let second = initialize_autostart(&mut store, true);
        assert_eq!(second, AutostartStatus::ok(true, false));
        assert_eq!(store.enable_new_calls, 1);
        assert_eq!(store.disable_old_calls, 1);
    }
}
