//! IPC metrics snapshot sent to the frontend.
//!
//! This is the CPU-only vertical slice. Memory, disk, network, GPU and
//! process fields are added in a later phase. Fields are `pub` so the struct
//! is part of the crate's public API and clippy does not flag dead code while
//! the sampler that produces it is still to come.

use serde::Serialize;
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
}
