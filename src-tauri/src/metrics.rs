//! IPC metrics snapshot sent to the frontend.
//!
//! This is the CPU-only vertical slice. Memory, disk, network, GPU and
//! process fields are added in a later phase. Fields are `pub` so the struct
//! is part of the crate's public API and clippy does not flag dead code while
//! the sampler that produces it is still to come.

use serde::Serialize;

/// A single point-in-time snapshot of system metrics, serialized to camelCase
/// JSON for the frontend.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MetricsSnapshot {
    /// Total CPU utilization, 0..=100 percent.
    pub cpu_total: f32,
    /// Per-core CPU utilization, 0..=100 percent each. Length equals the core
    /// count.
    pub cpu_per_core: Vec<f32>,
    /// Maximum per-core frequency in MHz. `None` when unavailable (serialized
    /// as `null`).
    pub cpu_freq_mhz: Option<u64>,
    /// Snapshot timestamp, epoch milliseconds.
    pub ts_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_serializes_camel_case() {
        let snapshot = MetricsSnapshot {
            cpu_total: 12.5,
            cpu_per_core: vec![1.0, 2.0],
            cpu_freq_mhz: Some(4800),
            ts_ms: 1,
        };
        let json = serde_json::to_string(&snapshot).expect("serialize");
        assert!(json.contains("\"cpuTotal\""));
        assert!(json.contains("\"cpuPerCore\""));
        assert!(json.contains("\"cpuFreqMhz\""));
        assert!(json.contains("\"tsMs\""));
        assert!(!json.contains("cpu_total"));
    }

    #[test]
    fn freq_none_serializes_null() {
        let snapshot = MetricsSnapshot {
            cpu_total: 0.0,
            cpu_per_core: vec![],
            cpu_freq_mhz: None,
            ts_ms: 0,
        };
        let json = serde_json::to_string(&snapshot).expect("serialize");
        assert!(json.contains("\"cpuFreqMhz\":null"));
    }
}
