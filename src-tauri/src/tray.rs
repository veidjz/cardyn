//! Menu-bar (tray) item: a static icon, a placeholder title and a minimal menu.
//!
//! This is the static tray built once on the main thread in `setup`. The live
//! title (CPU/RAM numbers) is wired in a later task, which fetches this tray by
//! [`TRAY_ID`] via `app.tray_by_id(...)` from the sampler thread, so the id must
//! stay a shared constant rather than a magic string.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager};

/// Stable id of the menu-bar tray, shared so a later task can retrieve the tray
/// (`app.tray_by_id(TRAY_ID)`) to update its title.
pub const TRAY_ID: &str = "main-tray";

/// Placeholder title shown until live values replace it (invariant 12: never a
/// false zero before the first real sample).
const TITLE_PLACEHOLDER: &str = "CPU -- RAM --";

/// Build the tray icon and attach it to the app.
///
/// Must run on the main thread (it is called from `setup`). The tray carries an
/// icon (mandatory on macOS or the item may not appear), the placeholder title
/// and a two-item menu. Left click shows the window; the menu opens on right
/// click. The built [`TrayIcon`](tauri::tray::TrayIcon) is owned by the app and
/// later reached by [`TRAY_ID`].
pub fn build(app: &App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Cardyn", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .title(TITLE_PLACEHOLDER)
        .menu(&menu)
        // Without this, the default left-click opens the menu; we want left
        // click to show the window and reserve the menu for right click.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // React only to a completed left click (button release) so hover,
            // move, enter and leave events never toggle the window.
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    // The configured default window icon doubles as the tray icon. Set it only
    // when present, so a missing icon degrades to no tray glyph instead of a
    // panic on startup.
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}

/// Show and focus the `main` window, tolerating its absence without panicking
/// (a later task may close it to the tray, leaving no window to show).
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
