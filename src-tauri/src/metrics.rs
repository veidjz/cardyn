//! IPC metrics snapshot sent to the frontend.
//!
//! This is the CPU-only vertical slice. Memory, disk, network, GPU and
//! process fields are added in a later phase. Fields are `pub` so the struct
//! is part of the crate's public API and clippy does not flag dead code while
//! the sampler that produces it is still to come.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::gpu::GpuSample;

/// A single process row for the "top processes" tables sent to the frontend.
/// `cpu_pct` is normalized to 0..=100 (the summed-across-cores reading divided
/// by the core count, ADR-013); `mem_bytes` is resident memory in bytes.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProcRow {
    /// Process identifier.
    pub pid: u32,
    /// Process name.
    pub name: String,
    /// Normalized CPU usage, 0..=100 percent.
    pub cpu_pct: f32,
    /// Resident memory, bytes.
    pub mem_bytes: u64,
}

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
    /// GPU reading for this tick. Each field is `None` ("GPU N/A") when the
    /// metric is unavailable; on Apple Silicon `vramTotal` is always `null`
    /// (unified memory, invariant 8).
    pub gpu: GpuSample,
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
    /// Network receive throughput, bytes/second, summed across physical
    /// interfaces over the last tick (loopback/VPN/virtual excluded; invariant
    /// 9). `0` when unavailable.
    pub net_rx_bps: u64,
    /// Network transmit throughput, bytes/second, summed across physical
    /// interfaces over the last tick (loopback/VPN/virtual excluded; invariant
    /// 9). `0` when unavailable.
    pub net_tx_bps: u64,
    /// Top 5 processes by normalized CPU usage, highest first (ADR-025). The
    /// full list is cut to the top N in the backend (invariant 3).
    pub top_by_cpu: Vec<ProcRow>,
    /// Top 5 processes by resident memory in bytes, highest first. The full
    /// list is cut to the top N in the backend (invariant 3).
    pub top_by_mem: Vec<ProcRow>,
    /// Snapshot timestamp, epoch milliseconds.
    pub ts_ms: u64,
}

/// Return the `n` rows with the highest `cpu_pct`, highest first. Pure.
pub fn top_by_cpu(rows: &[ProcRow], n: usize) -> Vec<ProcRow> {
    let mut sorted = rows.to_vec();
    // `total_cmp` gives a total order over f32 (NaN-safe), so a tie or a stray
    // NaN reading sorts deterministically instead of panicking.
    sorted.sort_by(|a, b| b.cpu_pct.total_cmp(&a.cpu_pct));
    sorted.truncate(n);
    sorted
}

/// Return the `n` rows with the highest `mem_bytes`, highest first. Pure.
pub fn top_by_mem(rows: &[ProcRow], n: usize) -> Vec<ProcRow> {
    let mut sorted = rows.to_vec();
    sorted.sort_by_key(|r| std::cmp::Reverse(r.mem_bytes));
    sorted.truncate(n);
    sorted
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

impl HistoryMetric {
    /// Every history series, in a fixed order. Used to iterate over all buffers
    /// when the sampler pushes a snapshot into the live history.
    pub const ALL: [HistoryMetric; 8] = [
        HistoryMetric::Cpu,
        HistoryMetric::Mem,
        HistoryMetric::GpuUtil,
        HistoryMetric::GpuMem,
        HistoryMetric::DiskRead,
        HistoryMetric::DiskWrite,
        HistoryMetric::NetRx,
        HistoryMetric::NetTx,
    ];
}

/// Raw series value for `metric` in `snap`, in the series' contract unit
/// (percent for `Cpu`/`GpuUtil`; BYTES for `Mem`/`GpuMem`; bytes/s for the disk
/// and net rates). `None` only when the underlying datum is absent: the GPU
/// reads are optional ("GPU N/A"), so they yield `None` when the GPU field is
/// `None` and no point is buffered that tick. Pure.
pub fn series_value(metric: HistoryMetric, snap: &MetricsSnapshot) -> Option<f64> {
    match metric {
        HistoryMetric::Cpu => Some(snap.cpu_total as f64),
        // `mem` stores used memory in BYTES (owner-confirmed), not a percent.
        HistoryMetric::Mem => Some(snap.mem_used as f64),
        HistoryMetric::GpuUtil => snap.gpu.utilization.map(|x| x as f64),
        HistoryMetric::GpuMem => snap.gpu.mem_used.map(|x| x as f64),
        HistoryMetric::DiskRead => Some(snap.disk_read_bps as f64),
        HistoryMetric::DiskWrite => Some(snap.disk_write_bps as f64),
        HistoryMetric::NetRx => Some(snap.net_rx_bps as f64),
        HistoryMetric::NetTx => Some(snap.net_tx_bps as f64),
    }
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

/// The live history for every series: one [`RingBuffer`] per [`HistoryMetric`],
/// each sized to the default live window (`RingBuffer::default()` is
/// `with_default_capacity()`). The sampler pushes one point per series per tick;
/// `get_history` reads one series out. Pure data structure: no time, IO or
/// threading.
#[derive(Default)]
pub struct Histories {
    cpu: RingBuffer,
    mem: RingBuffer,
    gpu_util: RingBuffer,
    gpu_mem: RingBuffer,
    disk_read: RingBuffer,
    disk_write: RingBuffer,
    net_rx: RingBuffer,
    net_tx: RingBuffer,
}

impl Histories {
    /// Mutable access to the buffer backing one series, for pushing new points.
    pub fn buffer_mut(&mut self, m: HistoryMetric) -> &mut RingBuffer {
        match m {
            HistoryMetric::Cpu => &mut self.cpu,
            HistoryMetric::Mem => &mut self.mem,
            HistoryMetric::GpuUtil => &mut self.gpu_util,
            HistoryMetric::GpuMem => &mut self.gpu_mem,
            HistoryMetric::DiskRead => &mut self.disk_read,
            HistoryMetric::DiskWrite => &mut self.disk_write,
            HistoryMetric::NetRx => &mut self.net_rx,
            HistoryMetric::NetTx => &mut self.net_tx,
        }
    }

    /// History snapshot for one series, oldest to newest.
    pub fn history(&self, m: HistoryMetric) -> History {
        match m {
            HistoryMetric::Cpu => self.cpu.history(),
            HistoryMetric::Mem => self.mem.history(),
            HistoryMetric::GpuUtil => self.gpu_util.history(),
            HistoryMetric::GpuMem => self.gpu_mem.history(),
            HistoryMetric::DiskRead => self.disk_read.history(),
            HistoryMetric::DiskWrite => self.disk_write.history(),
            HistoryMetric::NetRx => self.net_rx.history(),
            HistoryMetric::NetTx => self.net_tx.history(),
        }
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
            gpu: GpuSample {
                utilization: Some(42.0),
                mem_used: Some(8_000_000),
                vram_total: None,
            },
            disk_used: 250_000_000_000,
            disk_total: 500_000_000_000,
            disk_read_bps: 1_048_576,
            disk_write_bps: 524_288,
            net_rx_bps: 2_097_152,
            net_tx_bps: 131_072,
            top_by_cpu: vec![],
            top_by_mem: vec![],
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
        assert!(json.contains("\"gpu\""));
        assert!(json.contains("\"diskUsed\""));
        assert!(json.contains("\"diskTotal\""));
        assert!(json.contains("\"diskReadBps\""));
        assert!(json.contains("\"diskWriteBps\""));
        assert!(json.contains("\"netRxBps\""));
        assert!(json.contains("\"netTxBps\""));
        assert!(json.contains("\"topByCpu\""));
        assert!(json.contains("\"topByMem\""));
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
            gpu: GpuSample {
                utilization: None,
                mem_used: None,
                vram_total: None,
            },
            disk_used: 0,
            disk_total: 0,
            disk_read_bps: 0,
            disk_write_bps: 0,
            net_rx_bps: 0,
            net_tx_bps: 0,
            top_by_cpu: vec![],
            top_by_mem: vec![],
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

    fn proc(pid: u32, cpu: f32, mem: u64) -> ProcRow {
        ProcRow {
            pid,
            name: format!("p{pid}"),
            cpu_pct: cpu,
            mem_bytes: mem,
        }
    }

    #[test]
    fn top_by_cpu_orders_desc_and_truncates_to_n() {
        let rows = vec![
            proc(1, 10.0, 0),
            proc(2, 90.0, 0),
            proc(3, 50.0, 0),
            proc(4, 30.0, 0),
            proc(5, 70.0, 0),
            proc(6, 5.0, 0),
            proc(7, 99.0, 0),
        ];
        let top = top_by_cpu(&rows, 5);
        assert_eq!(top.len(), 5);
        let pids: Vec<u32> = top.iter().map(|r| r.pid).collect();
        assert_eq!(pids, vec![7, 2, 5, 3, 4]);
    }

    #[test]
    fn top_by_cpu_fewer_than_n_returns_all_sorted() {
        let rows = vec![proc(1, 10.0, 0), proc(2, 30.0, 0), proc(3, 20.0, 0)];
        let pids: Vec<u32> = top_by_cpu(&rows, 5).iter().map(|r| r.pid).collect();
        assert_eq!(pids, vec![2, 3, 1]);
    }

    #[test]
    fn top_by_cpu_empty_is_empty() {
        assert!(top_by_cpu(&[], 5).is_empty());
    }

    #[test]
    fn top_by_cpu_tie_does_not_panic() {
        let rows = vec![proc(1, 50.0, 0), proc(2, 50.0, 0), proc(3, 50.0, 0)];
        assert_eq!(top_by_cpu(&rows, 2).len(), 2);
    }

    #[test]
    fn top_by_mem_orders_desc_and_truncates_to_n() {
        let rows = vec![
            proc(1, 0.0, 100),
            proc(2, 0.0, 900),
            proc(3, 0.0, 500),
            proc(4, 0.0, 300),
            proc(5, 0.0, 700),
            proc(6, 0.0, 50),
            proc(7, 0.0, 999),
        ];
        let top = top_by_mem(&rows, 5);
        assert_eq!(top.len(), 5);
        let pids: Vec<u32> = top.iter().map(|r| r.pid).collect();
        assert_eq!(pids, vec![7, 2, 5, 3, 4]);
    }

    #[test]
    fn top_by_mem_fewer_than_n_returns_all_sorted() {
        let rows = vec![proc(1, 0.0, 10), proc(2, 0.0, 30), proc(3, 0.0, 20)];
        let pids: Vec<u32> = top_by_mem(&rows, 5).iter().map(|r| r.pid).collect();
        assert_eq!(pids, vec![2, 3, 1]);
    }

    #[test]
    fn top_by_mem_empty_is_empty() {
        assert!(top_by_mem(&[], 5).is_empty());
    }

    /// A snapshot with distinct, recognizable values per series so a mapping
    /// test can tell which field `series_value` read. `gpu` is parameterized so
    /// the "absent GPU datum" cases can pass `None`.
    fn sample_snapshot(gpu: GpuSample) -> MetricsSnapshot {
        MetricsSnapshot {
            cpu_total: 12.5,
            cpu_per_core: vec![],
            cpu_freq_mhz: None,
            mem_used: 8_000_000_000,
            mem_total: 16_000_000_000,
            mem_available: 0,
            mem_free: 0,
            swap_used: 0,
            swap_total: 0,
            gpu,
            disk_used: 0,
            disk_total: 0,
            disk_read_bps: 111,
            disk_write_bps: 222,
            net_rx_bps: 333,
            net_tx_bps: 444,
            top_by_cpu: vec![],
            top_by_mem: vec![],
            ts_ms: 7,
        }
    }

    #[test]
    fn series_value_maps_each_variant_to_its_field() {
        let snap = sample_snapshot(GpuSample {
            utilization: Some(42.0),
            mem_used: Some(9_000_000),
            vram_total: None,
        });
        assert_eq!(series_value(HistoryMetric::Cpu, &snap), Some(12.5));
        // `Mem` is BYTES, not a percent.
        assert_eq!(
            series_value(HistoryMetric::Mem, &snap),
            Some(8_000_000_000.0)
        );
        assert_eq!(series_value(HistoryMetric::GpuUtil, &snap), Some(42.0));
        assert_eq!(
            series_value(HistoryMetric::GpuMem, &snap),
            Some(9_000_000.0)
        );
        assert_eq!(series_value(HistoryMetric::DiskRead, &snap), Some(111.0));
        assert_eq!(series_value(HistoryMetric::DiskWrite, &snap), Some(222.0));
        assert_eq!(series_value(HistoryMetric::NetRx, &snap), Some(333.0));
        assert_eq!(series_value(HistoryMetric::NetTx, &snap), Some(444.0));
    }

    #[test]
    fn series_value_gpu_absent_is_none() {
        let snap = sample_snapshot(GpuSample {
            utilization: None,
            mem_used: None,
            vram_total: None,
        });
        assert_eq!(series_value(HistoryMetric::GpuUtil, &snap), None);
        assert_eq!(series_value(HistoryMetric::GpuMem, &snap), None);
        // Non-GPU series are always present even when the GPU is N/A.
        assert_eq!(series_value(HistoryMetric::Cpu, &snap), Some(12.5));
    }

    #[test]
    fn history_metric_all_has_eight_distinct_entries() {
        assert_eq!(HistoryMetric::ALL.len(), 8);
        for (i, a) in HistoryMetric::ALL.iter().enumerate() {
            for b in &HistoryMetric::ALL[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn histories_routes_each_metric_to_its_own_buffer() {
        let mut h = Histories::default();
        h.buffer_mut(HistoryMetric::Mem).push(1, 100.0);
        h.buffer_mut(HistoryMetric::Mem).push(2, 200.0);
        h.buffer_mut(HistoryMetric::NetRx).push(3, 9.0);

        let mem = h.history(HistoryMetric::Mem);
        assert_eq!(mem.t, vec![1, 2]);
        assert_eq!(mem.v, vec![100.0, 200.0]);

        let net = h.history(HistoryMetric::NetRx);
        assert_eq!(net.t, vec![3]);
        assert_eq!(net.v, vec![9.0]);

        // An untouched series stays empty.
        let cpu = h.history(HistoryMetric::Cpu);
        assert!(cpu.t.is_empty());
        assert!(cpu.v.is_empty());
    }
}
