//! Camel — Claude Code usage limits in the menu bar.
//!
//! Shape of the thing: a poller reads the JSON that the user's Claude Code
//! status line mirrors to disk, and turns it into two bars in the tray — the
//! 5-hour window and the week, coloured by how much is left — plus the worst
//! number as the tray title. Clicking the icon opens a small panel with the
//! full picture: both windows, reset times, data freshness. That's all the app
//! does: read one local file, draw. No network apart from the updater.

mod debug_log;
mod limits;
mod private;
mod tray_icon;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{
    menu::{MenuBuilder, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};
use tauri_plugin_updater::UpdaterExt;

/// How often the source file is re-read. It changes at most once per status
/// line render, so half a minute keeps the bars honest at no cost.
const POLL_SECS: u64 = 30;

const PANEL_W: f64 = 320.0;
/// Sized to the content: the update row only exists when a release is waiting,
/// and a fixed tall window would show dead air above the footer the rest of
/// the time.
const PANEL_H: f64 = 200.0;
const PANEL_H_WITH_UPDATE: f64 = 248.0;

struct AppState {
    snapshot: Mutex<Option<limits::Snapshot>>,
    update_badge: AtomicBool,
    update_version: Mutex<Option<String>>,
}

/// Everything the panel shows, in one payload: both windows, freshness, the
/// app version and a pending update if one was found.
#[tauri::command]
fn get_limits(state: tauri::State<AppState>) -> serde_json::Value {
    let snapshot = state.snapshot.lock().ok().and_then(|g| *g);
    let update = state.update_version.lock().ok().and_then(|g| g.clone());
    serde_json::json!({
        "snapshot": snapshot,
        "now": limits::now_secs(),
        "version": env!("CARGO_PKG_VERSION"),
        "update": update,
    })
}

#[tauri::command]
fn js_log(message: String) {
    debug_log::log(&format!("[ui] {}", message));
}

#[tauri::command]
fn hide_panel(app: AppHandle) {
    if let Some(w) = app.get_webview_window("panel") {
        let _ = w.hide();
    }
}

#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => {
            debug_log::log(&format!("update: downloading v{}", update.version));
            update
                .download_and_install(|_, _| {}, || debug_log::log("update: downloaded, restarting"))
                .await
                .map_err(|e| e.to_string())?;
            app.restart();
        }
        Ok(None) => Err("No updates available".into()),
        Err(e) => Err(e.to_string()),
    }
}

/// Re-read the source file and, when the numbers moved, redraw the tray and
/// tell the panel. Called from the poller and on panel open.
fn refresh(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let fresh = limits::read();
    let changed = match state.snapshot.lock() {
        Ok(mut g) => {
            let changed = *g != fresh;
            *g = fresh;
            changed
        }
        Err(_) => false,
    };
    if changed {
        apply_tray(app);
        let _ = app.emit("limits-changed", ());
    }
}

/// Draw the current numbers into the menu bar: bar icon plus the worst
/// remaining percent as the title. One function so the poller, the updater
/// badge and startup all paint the same way.
fn apply_tray(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let snapshot = state.snapshot.lock().ok().and_then(|g| *g);
    let badge = state.update_badge.load(Ordering::Relaxed);
    let bars = snapshot.map(|s| (s.five_hour.remaining, s.seven_day.remaining));
    let rgba = tray_icon::render(bars, badge);
    let icon =
        tauri::image::Image::new_owned(rgba, tray_icon::SIDE as u32, tray_icon::SIDE as u32);
    match app.tray_by_id("main") {
        Some(tray) => {
            if let Err(e) = tray.set_icon(Some(icon)) {
                debug_log::log(&format!("tray: set_icon failed: {}", e));
            }
            if let Err(e) = tray.set_title(snapshot.map(|s| format!("{}%", limits::worst(&s)))) {
                debug_log::log(&format!("tray: set_title failed: {}", e));
            }
            debug_log::log(&format!(
                "tray painted: bars {:?}, badge {}",
                bars, badge
            ));
        }
        None => debug_log::log("tray: no tray with id 'main'"),
    }
}

/// Light the badge and turn the menu's first item into the install action.
/// Called from both the menu check and the background poll.
fn announce_update(app: &AppHandle, version: &str) {
    if let Some(state) = app.try_state::<AppState>() {
        state.update_badge.store(true, Ordering::Relaxed);
        if let Ok(mut v) = state.update_version.lock() {
            *v = Some(version.to_string());
        }
    }
    if let Some(item) = app.try_state::<MenuItem<tauri::Wry>>() {
        let _ = item.set_text(format!("Update to v{}", version));
    }
    apply_tray(app);
    let _ = app.emit("update-available", version);
}

/// Left click on the tray: the panel appears right under the icon, or goes
/// away if it is up. The click rect arrives in physical pixels.
fn toggle_panel(app: &AppHandle, rect: &tauri::Rect) {
    let Some(window) = app.get_webview_window("panel") else { return };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    refresh(app);
    let panel_h = app
        .try_state::<AppState>()
        .map(|s| {
            if s.update_badge.load(Ordering::Relaxed) {
                PANEL_H_WITH_UPDATE
            } else {
                PANEL_H
            }
        })
        .unwrap_or(PANEL_H);
    let _ = window.set_size(tauri::LogicalSize::new(PANEL_W, panel_h));
    let scale = window.scale_factor().unwrap_or(1.0);
    let (px, py) = match (rect.position, rect.size) {
        (tauri::Position::Physical(p), tauri::Size::Physical(s)) => {
            (p.x as f64 / scale + s.width as f64 / scale / 2.0, p.y as f64 / scale + s.height as f64 / scale)
        }
        (tauri::Position::Logical(p), tauri::Size::Logical(s)) => (p.x + s.width / 2.0, p.y + s.height),
        _ => (0.0, 0.0),
    };
    // Centred under the icon on macOS; above the taskbar icon on Windows,
    // where the tray lives at the bottom of the screen.
    let x = (px - PANEL_W / 2.0).max(8.0);
    let y = if py > 400.0 { py - panel_h - 46.0 } else { py + 6.0 };
    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit("panel-opened", ());
}

pub fn run() {
    debug_log::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState {
            snapshot: Mutex::new(None),
            update_badge: AtomicBool::new(false),
            update_version: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            get_limits,
            js_log,
            hide_panel,
            install_update
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // Menu-bar utility: no Dock icon, no Cmd-Tab entry.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            build_tray(app)?;

            // First paint before the first poll tick.
            refresh(&handle);
            apply_tray(&handle);

            let poll_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(POLL_SECS)).await;
                    refresh(&poll_handle);
                }
            });

            // The app sits in the tray all day, so a release that ships while
            // it runs has to light the badge on its own.
            let update_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                loop {
                    if let Ok(updater) = update_handle.updater() {
                        match updater.check().await {
                            Ok(Some(update)) => {
                                debug_log::log(&format!("update: v{} available", update.version));
                                announce_update(&update_handle, &update.version);
                                break; // badge is lit — nothing left to poll for
                            }
                            Ok(None) => {}
                            Err(e) => debug_log::log(&format!("update: poll failed: {}", e)),
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
                }
            });

            debug_log::log("setup complete");
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "panel" {
                return;
            }
            match event {
                // The panel is a popover: focus elsewhere puts it away.
                tauri::WindowEvent::Focused(false) => {
                    let _ = window.hide();
                }
                // Closing must hide, never destroy — a destroyed panel cannot
                // be shown again and the tray icon would look dead.
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = window.hide();
                }
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Camel");
}

/// Right-click menu. Mirrors the other apps: update first, then the version,
/// then quit. The panel itself hangs on the left click.
fn build_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let update = MenuItem::with_id(app, "update", "Check for updates", true, None::<&str>)?;
    let version = MenuItem::with_id(
        app,
        "version",
        format!("Camel v{}", env!("CARGO_PKG_VERSION")),
        false,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", "Quit Camel", true, None::<&str>)?;

    let menu = MenuBuilder::new(app)
        .item(&update)
        .separator()
        .item(&version)
        .item(&quit)
        .build()?;

    // announce_update() rewrites this item's text when a release lands.
    app.manage(update.clone());

    // The icon goes in at build time: a status item created empty gets a
    // zero-width button on macOS, and later set_icon calls paint nothing the
    // user can see. Grey bars until the first snapshot lands a moment later.
    let initial = tauri::image::Image::new_owned(
        tray_icon::render(None, false),
        tray_icon::SIDE as u32,
        tray_icon::SIDE as u32,
    );
    TrayIconBuilder::with_id("main")
        .icon(initial)
        .icon_as_template(false)
        .tooltip("Camel — Claude Code limits")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "update" => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    on_update_clicked(app).await;
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = event
            {
                toggle_panel(tray.app_handle(), &rect);
            }
        })
        .build(app)?;
    Ok(())
}

/// One menu item, two jobs: check when nothing is pending, install once a
/// version has been found. Two items would leave a dead "Check" next to a
/// live "Update".
async fn on_update_clicked(app: AppHandle) {
    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            debug_log::log(&format!("update: no updater: {}", e));
            return;
        }
    };
    match updater.check().await {
        Ok(Some(update)) => {
            announce_update(&app, &update.version);
            if let Err(e) = install_update(app).await {
                debug_log::log(&format!("update: install failed: {}", e));
            }
        }
        Ok(None) => debug_log::log("update: up to date"),
        Err(e) => debug_log::log(&format!("update: check failed: {}", e)),
    }
}

#[cfg(test)]
mod window_tests {
    /// Windows the tray can raise; every one must be hidden on close, never
    /// destroyed. For Camel that is the panel alone — the test reads the
    /// config so a future second window cannot be forgotten here silently.
    const HIDDEN_ON_CLOSE: [&str; 1] = ["panel"];

    #[test]
    fn every_window_is_hidden_on_close_not_destroyed() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let labels = conf["app"]["windows"]
            .as_array()
            .expect("config has no windows")
            .iter()
            .map(|w| w["label"].as_str().expect("window without a label").to_string());

        for label in labels {
            assert!(
                HIDDEN_ON_CLOSE.contains(&label.as_str()),
                "window '{}' is not handled in on_window_event: closing it would \
                 destroy it, and the tray could never show it again",
                label
            );
        }
    }
}
