//! Menu-bar (tray) item: an icon, a live CPU/RAM title and a minimal menu.
//!
//! [`build`] runs once on the main thread in `setup` and seeds the title with a
//! placeholder. Each sampler tick then calls [`update_title`], which fetches this
//! tray by [`TRAY_ID`] via `app.tray_by_id(...)` from the sampler thread, so the
//! id must stay a shared constant rather than a magic string.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager};

/// Stable id of the menu-bar tray, shared so [`update_title`] can retrieve the
/// tray (`app.tray_by_id(TRAY_ID)`) to refresh its title each tick.
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

/// Format the live tray title from raw metric scalars.
///
/// Returns `"CPU {c}% RAM {m}%"`, where `c` is `cpu_total` and `m` is the memory
/// percentage (`mem_used / mem_total`), each rounded to the nearest integer and
/// clamped to `0..=100`. The memory ratio is computed in `f64` to avoid precision
/// loss and overflow on large byte counts.
///
/// When `mem_total` is `0` there is no real sample yet, so RAM renders as
/// `"RAM --"` rather than a false `0%` (invariant 12), matching the `--` style of
/// [`TITLE_PLACEHOLDER`] (e.g. `"CPU 12% RAM --"`).
pub fn format_title(cpu_total: f32, mem_used: u64, mem_total: u64) -> String {
    let cpu = (cpu_total.round() as i64).clamp(0, 100);
    if mem_total == 0 {
        return format!("CPU {cpu}% RAM --");
    }
    let mem = ((mem_used as f64 / mem_total as f64 * 100.0).round() as i64).clamp(0, 100);
    format!("CPU {cpu}% RAM {mem}%")
}

/// Update the live tray title from the latest metric scalars.
///
/// Fetches the tray by [`TRAY_ID`] and sets its title to [`format_title`]'s
/// output. A missing tray (`None`) and a transient `set_title` error are both
/// tolerated silently, so a single failure never tears down the sampler loop that
/// calls this once per tick.
pub fn update_title(app: &AppHandle, cpu_total: f32, mem_used: u64, mem_total: u64) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_title(Some(format_title(cpu_total, mem_used, mem_total)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_title_typical() {
        assert_eq!(format_title(12.0, 8_000, 16_000), "CPU 12% RAM 50%");
    }

    #[test]
    fn format_title_full_load() {
        assert_eq!(format_title(100.0, 16_000, 16_000), "CPU 100% RAM 100%");
    }

    #[test]
    fn format_title_zero_mem_total_renders_dashes() {
        assert_eq!(format_title(12.0, 0, 0), "CPU 12% RAM --");
    }

    #[test]
    fn format_title_rounds_to_nearest() {
        // CPU 12.6 -> 13; mem 5/8 = 62.5% -> 63 (round half away from zero).
        assert_eq!(format_title(12.6, 5, 8), "CPU 13% RAM 63%");
    }

    #[test]
    fn format_title_clamps_above_100() {
        assert_eq!(format_title(150.0, 20, 10), "CPU 100% RAM 100%");
    }
}
