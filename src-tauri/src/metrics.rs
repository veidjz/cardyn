//! IPC metrics snapshot sent to the frontend.
//!
//! This is the CPU-only vertical slice. Memory, disk, network, GPU and
//! process fields are added in a later phase. Fields are `pub` so the struct
//! is part of the crate's public API and clippy does not flag dead code while
//! the sampler that produces it is still to come.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

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
    /// Used physical memory, bytes. No app/wired/cache breakdown (ADR-023 /
    /// invariant 20).
    pub mem_used: u64,
    /// Total physical memory, bytes.
    pub mem_total: u64,
    /// Available physical memory, bytes (memory reclaimable for new allocations).
    pub mem_available: u64,
    /// Free physical memory, bytes (unused, not counting reclaimable caches).
    pub mem_free: u64,
    /// Used swap, bytes.
    pub swap_used: u64,
    /// Total swap, bytes.
    pub swap_total: u64,
    /// Used space on the system volume (`/`), bytes. `0` when the system volume
    /// is unavailable (sentinel; the frontend renders `--` when
    /// `disk_total == 0`).
    pub disk_used: u64,
    /// Total space on the system volume (`/`), bytes. `0` when the system volume
    /// is unavailable (sentinel).
    pub disk_total: u64,
    /// Disk read throughput, bytes/second, summed across physical disks over the
    /// last tick. `0` when unavailable.
    pub disk_read_bps: u64,
    /// Disk write throughput, bytes/second, summed across physical disks over the
    /// last tick. `0` when unavailable.
    pub disk_write_bps: u64,
    /// Snapshot timestamp, epoch milliseconds.
    pub ts_ms: u64,
}

/// A history snapshot of one metric series, in the uPlot-friendly parallel
/// column layout consumed over IPC. `t` holds epoch-millisecond timestamps and
/// `v` the matching series values; the two are always the same length and
/// aligned by index. Field names `t`/`v` are already the JSON keys per the IPC
/// contract, so no `rename_all` is needed.
#[derive(Serialize, Clone)]
pub struct History {
    /// Timestamps, epoch milliseconds, oldest to newest.
    pub t: Vec<u64>,
    /// Series values aligned with `t`.
    pub v: Vec<f64>,
}

/// The canonical set of history series the frontend can request over IPC. The
/// JSON values are camelCase (e.g. `"gpuUtil"`); an unknown value fails serde
/// deserialization at the IPC boundary, surfacing as a typed error rather than
/// a panic.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HistoryMetric {
    Cpu,
    Mem,
    GpuUtil,
    GpuMem,
    DiskRead,
    DiskWrite,
    NetRx,
    NetTx,
}

/// Number of points retained in the live history window (~60 seconds at 1 Hz).
pub const HISTORY_CAPACITY: usize = 60;

/// A bounded ring buffer of `(t, v)` points for one metric series. Timestamps
/// and values are stored in two `VecDeque`s kept in lockstep so they remain the
/// same length and index-aligned. When full, pushing a new point evicts the
/// oldest. Pure data structure: no time, IO or threading.
pub struct RingBuffer {
    cap: usize,
    t: VecDeque<u64>,
    v: VecDeque<f64>,
}

impl RingBuffer {
    /// Create an empty ring buffer holding at most `cap` points.
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            t: VecDeque::with_capacity(cap),
            v: VecDeque::with_capacity(cap),
        }
    }

    /// Create an empty ring buffer sized to the default live window.
    pub fn with_default_capacity() -> Self {
        Self::new(HISTORY_CAPACITY)
    }

    /// Append a `(t, v)` point. If the buffer is at capacity, the oldest point
    /// is evicted from both deques first so they stay aligned and bounded.
    pub fn push(&mut self, t: u64, v: f64) {
        if self.cap == 0 {
            return;
        }
        if self.t.len() == self.cap {
            self.t.pop_front();
            self.v.pop_front();
        }
        self.t.push_back(t);
        self.v.push_back(v);
    }

    /// Copy the retained points into a `History`, oldest to newest.
    pub fn history(&self) -> History {
        History {
            t: self.t.iter().copied().collect(),
            v: self.v.iter().copied().collect(),
        }
    }

    /// Number of points currently retained.
    pub fn len(&self) -> usize {
        self.t.len()
    }

    /// Whether the buffer holds no points.
    pub fn is_empty(&self) -> bool {
        self.t.is_empty()
    }
}

impl Default for RingBuffer {
    fn default() -> Self {
        Self::with_default_capacity()
    }
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
            mem_used: 8_000_000_000,
            mem_total: 16_000_000_000,
            mem_available: 7_000_000_000,
            mem_free: 6_000_000_000,
            swap_used: 1_000_000_000,
            swap_total: 2_000_000_000,
            disk_used: 250_000_000_000,
            disk_total: 500_000_000_000,
            disk_read_bps: 1_048_576,
            disk_write_bps: 524_288,
            ts_ms: 1,
        };
        let json = serde_json::to_string(&snapshot).expect("serialize");
        assert!(json.contains("\"cpuTotal\""));
        assert!(json.contains("\"cpuPerCore\""));
        assert!(json.contains("\"cpuFreqMhz\""));
        assert!(json.contains("\"memUsed\""));
        assert!(json.contains("\"memTotal\""));
        assert!(json.contains("\"memAvailable\""));
        assert!(json.contains("\"memFree\""));
        assert!(json.contains("\"swapUsed\""));
        assert!(json.contains("\"swapTotal\""));
        assert!(json.contains("\"diskUsed\""));
        assert!(json.contains("\"diskTotal\""));
        assert!(json.contains("\"diskReadBps\""));
        assert!(json.contains("\"diskWriteBps\""));
        assert!(json.contains("\"tsMs\""));
        assert!(!json.contains("cpu_total"));
        assert!(!json.contains("mem_used"));
        assert!(!json.contains("swap_total"));
        assert!(!json.contains("disk_used"));
        assert!(!json.contains("disk_read_bps"));
    }

    #[test]
    fn freq_none_serializes_null() {
        let snapshot = MetricsSnapshot {
            cpu_total: 0.0,
            cpu_per_core: vec![],
            cpu_freq_mhz: None,
            mem_used: 0,
            mem_total: 0,
            mem_available: 0,
            mem_free: 0,
            swap_used: 0,
            swap_total: 0,
            disk_used: 0,
            disk_total: 0,
            disk_read_bps: 0,
            disk_write_bps: 0,
            ts_ms: 0,
        };
        let json = serde_json::to_string(&snapshot).expect("serialize");
        assert!(json.contains("\"cpuFreqMhz\":null"));
    }

    #[test]
    fn ring_buffer_evicts_oldest_beyond_capacity() {
        let mut rb = RingBuffer::new(3);
        for i in 1..=5u64 {
            rb.push(i, i as f64);
        }
        let h = rb.history();
        assert_eq!(rb.len(), 3);
        assert_eq!(h.t, vec![3, 4, 5]);
        assert_eq!(h.v, vec![3.0, 4.0, 5.0]);
    }

    #[test]
    fn ring_buffer_keeps_columns_aligned() {
        let mut rb = RingBuffer::new(2);
        for i in 0..10u64 {
            rb.push(i, (i * 10) as f64);
        }
        let h = rb.history();
        assert_eq!(h.t.len(), h.v.len());
        assert_eq!(h.t.len(), 2);
        assert_eq!(h.t, vec![8, 9]);
        assert_eq!(h.v, vec![80.0, 90.0]);
    }

    #[test]
    fn ring_buffer_history_empty_when_unfilled() {
        let rb = RingBuffer::new(4);
        let h = rb.history();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
        assert!(h.t.is_empty());
        assert!(h.v.is_empty());
    }

    #[test]
    fn ring_buffer_under_capacity_keeps_insertion_order() {
        let mut rb = RingBuffer::new(5);
        rb.push(10, 1.5);
        rb.push(20, 2.5);
        rb.push(30, 3.5);
        let h = rb.history();
        assert_eq!(rb.len(), 3);
        assert_eq!(h.t, vec![10, 20, 30]);
        assert_eq!(h.v, vec![1.5, 2.5, 3.5]);
    }

    #[test]
    fn ring_buffer_default_uses_history_capacity() {
        let rb = RingBuffer::default();
        assert_eq!(rb.len(), 0);
        let mut rb = RingBuffer::with_default_capacity();
        for i in 0..(HISTORY_CAPACITY as u64 + 10) {
            rb.push(i, i as f64);
        }
        assert_eq!(rb.len(), HISTORY_CAPACITY);
    }

    #[test]
    fn history_metric_deserializes_camel_case() {
        assert_eq!(
            serde_json::from_str::<HistoryMetric>("\"cpu\"").unwrap(),
            HistoryMetric::Cpu
        );
        assert_eq!(
            serde_json::from_str::<HistoryMetric>("\"gpuUtil\"").unwrap(),
            HistoryMetric::GpuUtil
        );
        assert_eq!(
            serde_json::from_str::<HistoryMetric>("\"netRx\"").unwrap(),
            HistoryMetric::NetRx
        );
    }

    #[test]
    fn history_metric_rejects_unknown_value() {
        assert!(serde_json::from_str::<HistoryMetric>("\"bogus\"").is_err());
    }
}
