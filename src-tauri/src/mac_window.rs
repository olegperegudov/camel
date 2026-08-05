//! The panel is a non-activating NSPanel, not a window.
//!
//! An ordinary window belonging to an Accessory app never made it onto the
//! screen here: `show()` placed it at the right coordinates and the window
//! server kept it ordered out (visible per Tauri, absent per CGWindowList).
//! A floating panel — the mechanism Spotlight uses, and the same one Iago
//! ships — surfaces regardless of app activation, over full-screen Spaces too.

#[cfg(target_os = "macos")]
use tauri::Manager as _;

#[cfg(target_os = "macos")]
tauri_nspanel::tauri_panel! {
    panel!(CamelPanel {
        config: {
            can_become_key_window: true,   // Esc closes the panel
            can_become_main_window: false,
            is_floating_panel: true
        }
    })
}

#[cfg(target_os = "macos")]
pub fn setup_panel(window: &tauri::WebviewWindow) -> Result<(), String> {
    use tauri_nspanel::{CollectionBehavior, StyleMask, WebviewWindowExt};

    let panel = window.to_panel::<CamelPanel>().map_err(|e| e.to_string())?;
    panel.set_style_mask(StyleMask::empty().nonactivating_panel().into());
    panel.set_collection_behavior(
        CollectionBehavior::new()
            .full_screen_auxiliary()
            .can_join_all_spaces()
            .into(),
    );
    // The app is never active (Accessory, by design) — deactivation must not
    // put the panel away, the outside-click monitor does that.
    panel.set_hides_on_deactivate(false);
    crate::debug_log::log("panel: converted to non-activating NSPanel");
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn show_panel(app: &tauri::AppHandle) {
    use tauri_nspanel::ManagerExt;
    match app.get_webview_panel("panel") {
        Ok(p) => p.show_and_make_key(),
        Err(e) => crate::debug_log::log(&format!("show_panel: panel missing ({:?})", e)),
    }
}

#[cfg(target_os = "macos")]
pub fn hide_panel(app: &tauri::AppHandle) {
    use tauri_nspanel::ManagerExt;
    if let Ok(p) = app.get_webview_panel("panel") {
        p.hide();
    }
}

#[cfg(target_os = "macos")]
pub fn panel_visible(app: &tauri::AppHandle) -> bool {
    use tauri_nspanel::ManagerExt;
    app.get_webview_panel("panel").map(|p| p.is_visible()).unwrap_or(false)
}

/// Dismisses the panel on a click anywhere outside it. A non-activating panel
/// never gets a "focus lost" callback — a global NSEvent monitor reports
/// mouse-downs landing in *other* apps and never fires for clicks inside our
/// own window. Mouse monitors need no Accessibility grant.
#[cfg(target_os = "macos")]
pub fn dismiss_on_outside_click(app: tauri::AppHandle) {
    use block::ConcreteBlock;
    use cocoa::base::id;
    use objc::{class, msg_send, sel, sel_impl};

    const LEFT_MOUSE_DOWN: u64 = 1 << 1;
    const RIGHT_MOUSE_DOWN: u64 = 1 << 3;
    const OTHER_MOUSE_DOWN: u64 = 1 << 25;

    let handler = ConcreteBlock::new(move |_event: id| {
        if panel_visible(&app) {
            hide_panel(&app);
        }
    });
    // The monitor outlives this call and keeps calling the block, so the block
    // has to outlive it too — copied to the heap and deliberately never freed.
    let handler = handler.copy();
    unsafe {
        let mask = LEFT_MOUSE_DOWN | RIGHT_MOUSE_DOWN | OTHER_MOUSE_DOWN;
        let _: id = msg_send![class!(NSEvent),
            addGlobalMonitorForEventsMatchingMask: mask
            handler: &*handler];
    }
    std::mem::forget(handler);
    crate::debug_log::log("panel: watching for clicks outside");
}

#[cfg(not(target_os = "macos"))]
pub fn setup_panel(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn show_panel(app: &tauri::AppHandle) {
    use tauri::Manager as _;
    if let Some(w) = app.get_webview_window("panel") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[cfg(not(target_os = "macos"))]
pub fn hide_panel(app: &tauri::AppHandle) {
    use tauri::Manager as _;
    if let Some(w) = app.get_webview_window("panel") {
        let _ = w.hide();
    }
}

#[cfg(not(target_os = "macos"))]
pub fn panel_visible(app: &tauri::AppHandle) -> bool {
    use tauri::Manager as _;
    app.get_webview_window("panel")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

// Windows needs no monitor: the panel there is an ordinary window, and the
// focus handler in lib.rs hides it when another window takes over.
#[cfg(not(target_os = "macos"))]
pub fn dismiss_on_outside_click(_app: tauri::AppHandle) {}
