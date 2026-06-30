use std::sync::Mutex;

use tauri::Manager;

use crate::metrics::{Histories, MetricsSnapshot};

pub mod gpu;
pub mod metrics;
pub mod sampler;
mod tray;

/// Shared application state behind the Tauri managed-state registry.
///
/// Both fields are guarded by a `Mutex` so the sampler thread and any IPC
/// command can read or update them concurrently. `last` holds the most recent
/// snapshot (`None` until the first tick); `histories` holds one bounded ring
/// buffer per metric series, feeding the live history window.
pub struct AppState {
    pub last: Mutex<Option<MetricsSnapshot>>,
    pub histories: Mutex<Histories>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            last: Mutex::new(None),
            histories: Mutex::new(Histories::default()),
        }
    }
}

/// Return the live history for one metric series, reading the series' ring
/// buffer. A series with no points yet reports an empty history. An invalid
/// `metric` value fails deserialization at the IPC boundary and never reaches
/// this function. Mutex poisoning is tolerated: a poisoned guard's inner value
/// is still a valid history to read.
#[tauri::command]
fn get_history(
    metric: crate::metrics::HistoryMetric,
    state: tauri::State<AppState>,
) -> crate::metrics::History {
    state
        .histories
        .lock()
        .map(|h| h.history(metric))
        .unwrap_or_else(|p| p.into_inner().history(metric))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    // The single-instance plugin must be registered before any other plugin so
    // a second launch focuses the running window instead of spawning a new app.
    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // The window may be hidden behind a future close-to-tray; show it
            // before focusing, and tolerate a missing window without panicking.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
    }

    builder
        // Close-to-tray: the red close button hides the main window instead of
        // quitting, keeping the sampler and tray alive. The app quits only via
        // the tray "Quit" item or macOS Cmd+Q, neither of which is a window
        // CloseRequested. Scoped to "main" so future windows are unaffected.
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            app.manage(AppState::default());
            // Built on the main thread here; a later task reaches it by id from
            // the sampler thread to update the title.
            tray::build(app)?;
            sampler::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_history])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Clicking the macOS Dock icon of a running app fires a reopen event;
            // show and focus the main window so it returns after close-to-tray hid
            // it. Tolerate a missing window without panicking. Showing an already
            // visible window is a harmless no-op, so there is no need to branch on
            // has_visible_windows.
            if let tauri::RunEvent::Reopen { .. } = event {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        });
}
