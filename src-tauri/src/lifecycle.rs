use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager};

/// Process-level exit flag so close-to-tray can distinguish hide vs real quit.
pub struct LifecycleState {
    exiting: AtomicBool,
}

impl Default for LifecycleState {
    fn default() -> Self {
        Self {
            exiting: AtomicBool::new(false),
        }
    }
}

impl LifecycleState {
    pub fn request_exit(&self) {
        self.exiting.store(true, Ordering::SeqCst);
    }

    pub fn is_exiting(&self) -> bool {
        self.exiting.load(Ordering::SeqCst)
    }
}

/// Shared restore path for tray Open, tray double-click, and second-instance focus.
/// Failures are ignored so later steps still run.
pub fn show_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
}
