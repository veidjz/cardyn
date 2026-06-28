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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(AppState::default());
            sampler::spawn(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
