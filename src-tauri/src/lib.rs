use std::sync::Mutex;

use tauri::Manager;

use crate::metrics::{MetricsSnapshot, RingBuffer};

pub mod gpu;
pub mod metrics;
pub mod sampler;

/// Shared application state behind the Tauri managed-state registry.
///
/// Both fields are guarded by a `Mutex` so the sampler thread and any IPC
/// command can read or update them concurrently. `last` holds the most recent
/// snapshot (`None` until the first tick); `cpu_history` is the bounded ring
/// buffer feeding the live history window.
pub struct AppState {
    pub last: Mutex<Option<MetricsSnapshot>>,
    pub cpu_history: Mutex<RingBuffer>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            last: Mutex::new(None),
            cpu_history: Mutex::new(RingBuffer::with_default_capacity()),
        }
    }
}

/// Return the live history for one metric series. Only `Cpu` is backed by a
/// ring buffer so far; the remaining series get real buffers in a later phase
/// and report an empty history until then. An invalid `metric` value fails
/// deserialization at the IPC boundary and never reaches this function.
#[tauri::command]
fn get_history(
    metric: crate::metrics::HistoryMetric,
    state: tauri::State<AppState>,
) -> crate::metrics::History {
    use crate::metrics::{History, HistoryMetric};
    match metric {
        HistoryMetric::Cpu => state
            .cpu_history
            .lock()
            .map(|h| h.history())
            .unwrap_or_else(|p| p.into_inner().history()),
        // Other series get real ring buffers in P4; until then report empty history.
        _ => History {
            t: Vec::new(),
            v: Vec::new(),
        },
    }
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
        .setup(|app| {
            app.manage(AppState::default());
            sampler::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_history])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
