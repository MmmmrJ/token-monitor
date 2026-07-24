use crate::coordinator::MonitorCoordinator;
use crate::lifecycle::{show_main_window, LifecycleState};
use crate::snapshot::MonitorSnapshot;
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Emitter, Manager, Wry,
};

#[cfg(target_os = "macos")]
fn tray_icon_image() -> tauri::Result<Image<'static>> {
    const SIZE: u32 = 32;
    let mut rgba = vec![0_u8; (SIZE * SIZE * 4) as usize];
    let bars = [
        (5.0_f32, 14.0_f32, 10.0_f32, 27.0_f32),
        (13.0, 6.0, 18.0, 27.0),
        (21.0, 17.0, 26.0, 27.0),
    ];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let visible = bars.iter().any(|&(left, top, right, bottom)| {
                let radius = 2.5_f32;
                let closest_x = px.clamp(left + radius, right - radius);
                let closest_y = py.clamp(top + radius, bottom - radius);
                let dx = px - closest_x;
                let dy = py - closest_y;
                dx * dx + dy * dy <= radius * radius
            });
            if visible {
                let offset = ((y * SIZE + x) * 4) as usize;
                rgba[offset + 3] = u8::MAX;
            }
        }
    }

    Ok(Image::new_owned(rgba, SIZE, SIZE))
}

#[cfg(not(target_os = "macos"))]
fn tray_icon_image() -> tauri::Result<Image<'static>> {
    Image::from_bytes(include_bytes!("../icons/32x32.png"))
}

fn countdown_label(snapshot: &MonitorSnapshot, language: &str) -> String {
    let mut candidates = Vec::new();
    if let Some(window) = snapshot.windows.five_hour.as_ref() {
        if let Some(reset) = window.resets_at.as_ref() {
            candidates.push(reset.clone());
        }
    }
    if let Some(window) = snapshot.windows.seven_day.as_ref() {
        if let Some(reset) = window.resets_at.as_ref() {
            candidates.push(reset.clone());
        }
    }
    candidates.sort();
    let Some(next) = candidates.first() else {
        return if language == "en" {
            "No reset time".into()
        } else {
            "无重置时间".into()
        };
    };
    if language == "en" {
        format!("Next reset {next}")
    } else {
        format!("下次重置 {next}")
    }
}

fn summary_lines(snapshot: &MonitorSnapshot, language: &str) -> (String, String, String) {
    let kind = snapshot.provider.kind.as_str();
    let (primary, secondary) = if kind == "cursor" {
        if language == "en" {
            ("First-party", "API")
        } else {
            ("订阅额度", "API 额度")
        }
    } else if language == "en" {
        ("5-hour", "7-day")
    } else {
        ("5 小时", "7 天")
    };
    let primary_line = snapshot
        .windows
        .five_hour
        .as_ref()
        .map(|window| format!("{primary} {:.0}%", window.remaining_percent))
        .unwrap_or_else(|| format!("{primary} —"));
    let secondary_line = snapshot
        .windows
        .seven_day
        .as_ref()
        .map(|window| format!("{secondary} {:.0}%", window.remaining_percent))
        .unwrap_or_else(|| format!("{secondary} —"));
    (
        primary_line,
        secondary_line,
        countdown_label(snapshot, language),
    )
}

pub async fn update_tray_from_snapshot(app: &AppHandle, snapshot: &MonitorSnapshot) {
    let language = app
        .state::<MonitorCoordinator>()
        .ui
        .lock()
        .await
        .language
        .clone();
    let (primary, secondary, reset) = summary_lines(snapshot, &language);
    if let Some(tray) = app.tray_by_id("monitor-tray") {
        let _ = tray.set_tooltip(Some(format!(
            "Token Monitor · {primary} · {secondary} · {reset}"
        )));
    }
    rebuild_tray_menu(app).await;
}

pub async fn rebuild_tray_menu(app: &AppHandle) {
    let ui = app.state::<MonitorCoordinator>().ui.lock().await.clone();
    let language = ui.language.as_str();
    let snapshot = app
        .state::<crate::snapshot::MonitorState>()
        .snapshots
        .lock()
        .ok()
        .and_then(|guard| guard.get(&ui.provider).cloned());

    let (summary_a, summary_b, reset) = snapshot
        .as_ref()
        .map(|item| summary_lines(item, language))
        .unwrap_or_else(|| {
            if language == "en" {
                (
                    "Usage unavailable".into(),
                    "—".into(),
                    "No reset time".into(),
                )
            } else {
                ("暂无额度".into(), "—".into(), "无重置时间".into())
            }
        });

    let t = |zh: &str, en: &str| -> String {
        if language == "en" {
            en.to_string()
        } else {
            zh.to_string()
        }
    };

    let Ok(summary1) = MenuItem::with_id(app, "summary_primary", summary_a, false, None::<&str>)
    else {
        return;
    };
    let Ok(summary2) = MenuItem::with_id(app, "summary_secondary", summary_b, false, None::<&str>)
    else {
        return;
    };
    let Ok(summary3) = MenuItem::with_id(app, "summary_reset", reset, false, None::<&str>) else {
        return;
    };
    let Ok(sep1) = PredefinedMenuItem::separator(app) else {
        return;
    };
    let Ok(provider_codex) = CheckMenuItem::with_id(
        app,
        "provider_codex",
        "Codex",
        true,
        ui.provider == "codex",
        None::<&str>,
    ) else {
        return;
    };
    let Ok(provider_cursor) = CheckMenuItem::with_id(
        app,
        "provider_cursor",
        "Cursor",
        true,
        ui.provider == "cursor",
        None::<&str>,
    ) else {
        return;
    };
    let Ok(view_dual) = CheckMenuItem::with_id(
        app,
        "view_dual",
        t("双环视图", "Dual rings"),
        true,
        ui.view == "dual",
        None::<&str>,
    ) else {
        return;
    };
    let Ok(view_focus) = CheckMenuItem::with_id(
        app,
        "view_focus",
        t("聚焦视图", "Focus view"),
        true,
        ui.view == "focus",
        None::<&str>,
    ) else {
        return;
    };
    let Ok(refresh) = MenuItem::with_id(
        app,
        "refresh",
        t("刷新额度", "Refresh usage"),
        true,
        None::<&str>,
    ) else {
        return;
    };
    let Ok(always_on_top) = CheckMenuItem::with_id(
        app,
        "always_on_top",
        t("始终置顶", "Always on top"),
        true,
        ui.always_on_top,
        None::<&str>,
    ) else {
        return;
    };
    let Ok(show) = MenuItem::with_id(
        app,
        "show",
        t("显示窗口", "Show window"),
        true,
        None::<&str>,
    ) else {
        return;
    };
    let Ok(settings) = MenuItem::with_id(
        app,
        "settings",
        t("打开设置", "Open settings"),
        true,
        None::<&str>,
    ) else {
        return;
    };
    let Ok(quit) = MenuItem::with_id(app, "quit", t("退出", "Quit"), true, None::<&str>) else {
        return;
    };

    let Ok(menu) = Menu::with_items(
        app,
        &[
            &summary1,
            &summary2,
            &summary3,
            &sep1,
            &provider_codex,
            &provider_cursor,
            &view_dual,
            &view_focus,
            &refresh,
            &always_on_top,
            &show,
            &settings,
            &quit,
        ],
    ) else {
        return;
    };

    if let Some(tray) = app.tray_by_id("monitor-tray") {
        let _ = tray.set_menu(Some(menu));
    }
}

pub fn setup_tray(app: &App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Open Token Monitor", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh usage", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &refresh, &quit])?;
    TrayIconBuilder::with_id("monitor-tray")
        .icon(tray_icon_image()?)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Token Monitor")
        .menu(&menu)
        .on_menu_event(|app, event| {
            let id = event.id.as_ref().to_string();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                handle_tray_menu(&app, &id).await;
            });
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::DoubleClick { .. } => {
                show_main_window(tray.app_handle());
            }
            TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } => {
                #[cfg(target_os = "windows")]
                {
                    show_main_window(tray.app_handle());
                }
            }
            _ => {}
        })
        .build(app)?;
    Ok(())
}

async fn handle_tray_menu(app: &AppHandle<Wry>, id: &str) {
    match id {
        "show" => show_main_window(app),
        "settings" => {
            show_main_window(app);
            let _ = app.emit("monitor:open-settings", ());
        }
        "refresh" => {
            let provider = app
                .state::<MonitorCoordinator>()
                .ui
                .lock()
                .await
                .provider
                .clone();
            let _ = app
                .state::<MonitorCoordinator>()
                .refresh(app, Some(&provider), true)
                .await;
        }
        "provider_codex" | "provider_cursor" => {
            let provider = if id == "provider_cursor" {
                "cursor"
            } else {
                "codex"
            };
            {
                let coordinator = app.state::<MonitorCoordinator>();
                coordinator.ui.lock().await.provider = provider.into();
            }
            let _ = app.emit("monitor:set-provider", provider);
            let _ = app
                .state::<MonitorCoordinator>()
                .refresh(app, Some(provider), true)
                .await;
            rebuild_tray_menu(app).await;
        }
        "view_dual" | "view_focus" => {
            let view = if id == "view_focus" { "focus" } else { "dual" };
            app.state::<MonitorCoordinator>().ui.lock().await.view = view.into();
            let _ = app.emit("monitor:set-view", view);
            rebuild_tray_menu(app).await;
        }
        "always_on_top" => {
            let enabled = {
                let coordinator = app.state::<MonitorCoordinator>();
                let mut ui = coordinator.ui.lock().await;
                ui.always_on_top = !ui.always_on_top;
                ui.always_on_top
            };
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(enabled);
            }
            let _ = app.emit("monitor:set-always-on-top", enabled);
            rebuild_tray_menu(app).await;
        }
        "quit" => {
            app.state::<LifecycleState>().request_exit();
            app.exit(0);
        }
        _ => {}
    }
}
