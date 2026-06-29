//! CPU sampler backed by a long-lived `sysinfo::System`.
//!
//! This is the CPU-only vertical slice. The sampler keeps one `System` and a
//! scoped [`CpuRefreshKind`] alive for the process lifetime so each [`tick`]
//! reuses the same allocation and refresh scope, rather than rebuilding state
//! every sample. Memory, disk, network and GPU reads are added in a later phase.
//!
//! The sampler never sleeps internally: the caller owns timing. CPU usage is a
//! delta between two refreshes, so callers must space ticks by at least
//! [`sysinfo::MINIMUM_CPU_UPDATE_INTERVAL`] (~200 ms) to get meaningful values.
//! Items are `pub` so clippy does not flag dead code while the Tauri wiring that
//! drives the sampler is still to come.
//!
//! [`tick`]: Sampler::tick

use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, System};
use tauri::{AppHandle, Emitter, Manager};

use crate::metrics::MetricsSnapshot;

/// Samples CPU utilization and frequency from a long-lived `sysinfo::System`.
pub struct Sampler {
    /// Reused across ticks so CPU usage is computed as a delta between samples.
    system: System,
    /// Scoped refresh kind (CPU usage + frequency only), stored so every tick
    /// refreshes exactly the same fields.
    cpu_refresh: CpuRefreshKind,
    /// Scoped refresh kind (RAM + swap only), stored so every tick refreshes
    /// exactly the same fields.
    mem_refresh: MemoryRefreshKind,
    /// Long-lived disk list, reused across ticks so cumulative I/O counters can
    /// be diffed into a throughput rate.
    disks: Disks,
    /// Previous cumulative bytes read across all disks, for the rate delta.
    prev_disk_read: u64,
    /// Previous cumulative bytes written across all disks, for the rate delta.
    prev_disk_write: u64,
    /// Timestamp of the previous disk sample, epoch milliseconds, for the rate
    /// delta's `dt`.
    prev_ts_ms: u64,
}

impl Sampler {
    /// Create a sampler with a single discardable warm-up refresh.
    ///
    /// `sysinfo`'s very first CPU sample is always `0` (there is no prior sample
    /// to diff against), so we take and discard one refresh here. The first real
    /// [`tick`](Self::tick) then returns a meaningful delta, provided the caller
    /// waited at least [`sysinfo::MINIMUM_CPU_UPDATE_INTERVAL`] beforehand.
    pub fn new() -> Self {
        // `new` (not `new_all`): we only ever read CPU, so we avoid the upfront
        // cost of enumerating processes, disks and networks.
        let mut system = System::new();
        let cpu_refresh = CpuRefreshKind::nothing().with_cpu_usage().with_frequency();
        let mem_refresh = MemoryRefreshKind::nothing().with_ram().with_swap();
        // Warm-up: prime the baseline so the first tick is a real delta.
        system.refresh_cpu_specifics(cpu_refresh);
        // Warm-up: prime memory so the first tick already has values. Memory is
        // an absolute reading (not a delta), but priming keeps tick uniform.
        system.refresh_memory_specifics(mem_refresh);
        // Disk list is refreshed on construction, so the cumulative I/O counters
        // are already populated.
        let disks = Disks::new_with_refreshed_list();
        let (prev_disk_read, prev_disk_write) = disk_io_totals(&disks);
        Self {
            system,
            cpu_refresh,
            mem_refresh,
            disks,
            // Seed from the current cumulative totals so the first tick reports a
            // real rate, not a spike from a zero baseline.
            prev_disk_read,
            prev_disk_write,
            prev_ts_ms: now_ms(),
        }
    }

    /// Refresh CPU state and return a fresh [`MetricsSnapshot`].
    ///
    /// Does not sleep; the caller controls cadence. Values reflect the interval
    /// since the previous refresh (construction or the prior tick).
    pub fn tick(&mut self) -> MetricsSnapshot {
        self.system.refresh_cpu_specifics(self.cpu_refresh);
        self.system.refresh_memory_specifics(self.mem_refresh);
        // Refresh space + I/O for the existing disks; `true` drops disks that
        // disappeared so stale entries do not skew the I/O sum.
        self.disks.refresh(true);

        let cpu_total = self.system.global_cpu_usage();
        let cpu_per_core: Vec<f32> = self
            .system
            .cpus()
            .iter()
            .map(|cpu| cpu.cpu_usage())
            .collect();
        let per_core_freqs: Vec<u64> = self
            .system
            .cpus()
            .iter()
            .map(|cpu| cpu.frequency())
            .collect();

        // SPACE: system volume mounted at `/`. Missing `/` -> 0/0 sentinel.
        let root = Path::new("/");
        let (disk_used, disk_total) = self
            .disks
            .list()
            .iter()
            .find(|disk| disk.mount_point() == root)
            .map(|disk| {
                let total = disk.total_space();
                (total.saturating_sub(disk.available_space()), total)
            })
            .unwrap_or((0, 0));

        // THROUGHPUT: diff cumulative I/O counters over the elapsed time.
        let (now_read, now_write) = disk_io_totals(&self.disks);
        let snapshot_ts_ms = now_ms();
        let dt_secs = (snapshot_ts_ms.saturating_sub(self.prev_ts_ms)) as f64 / 1000.0;
        let disk_read_bps = bytes_per_sec(self.prev_disk_read, now_read, dt_secs);
        let disk_write_bps = bytes_per_sec(self.prev_disk_write, now_write, dt_secs);
        self.prev_disk_read = now_read;
        self.prev_disk_write = now_write;
        self.prev_ts_ms = snapshot_ts_ms;

        MetricsSnapshot {
            cpu_total,
            cpu_per_core,
            cpu_freq_mhz: max_freq_mhz(&per_core_freqs),
            mem_used: self.system.used_memory(),
            mem_total: self.system.total_memory(),
            mem_available: self.system.available_memory(),
            mem_free: self.system.free_memory(),
            swap_used: self.system.used_swap(),
            swap_total: self.system.total_swap(),
            disk_used,
            disk_total,
            disk_read_bps,
            disk_write_bps,
            ts_ms: snapshot_ts_ms,
        }
    }
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}

/// Spawn the background sampler thread that drives the live metrics feed.
///
/// The thread owns a [`Sampler`], samples at ~1 Hz, stores the latest snapshot
/// and pushes CPU total into the shared ring buffer (both behind the managed
/// [`AppState`](crate::AppState) mutexes), then emits a `metrics` event each
/// tick. Nothing here may panic: mutex poisoning and emit errors are tolerated
/// so a transient failure never tears down the loop.
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut sampler = Sampler::new();
        // Let the warm-up baseline age before the first real read so the first
        // tick is a meaningful delta rather than the sysinfo zero sample.
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        loop {
            let snapshot = sampler.tick();
            // `State` is fetched per lock rather than bound to a shared local:
            // the `if let` scrutinee temporary borrows it, and inlining keeps the
            // guard and its `State` dropping together (edition 2021 drop order).
            if let Ok(mut history) = app.state::<crate::AppState>().cpu_history.lock() {
                history.push(snapshot.ts_ms, snapshot.cpu_total as f64);
            }
            if let Ok(mut last) = app.state::<crate::AppState>().last.lock() {
                *last = Some(snapshot.clone());
            }
            let _ = app.emit("metrics", snapshot);
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

/// Highest per-core frequency in MHz, or `None` when the slice is empty or every
/// core reports `0` (i.e. the platform did not supply a frequency).
fn max_freq_mhz(per_core: &[u64]) -> Option<u64> {
    per_core.iter().copied().max().filter(|&m| m > 0)
}

/// Sum of cumulative `(read, written)` bytes across all listed disks, deduped by
/// name. Cumulative since boot; diff two samples over time to get a rate. Not
/// pure (reads the disk list), so timing/IO stays out of [`bytes_per_sec`] and
/// [`sum_distinct_io`].
fn disk_io_totals(disks: &Disks) -> (u64, u64) {
    let named: Vec<(String, u64, u64)> = disks
        .list()
        .iter()
        .map(|disk| {
            let usage = disk.usage();
            (
                disk.name().to_string_lossy().into_owned(),
                usage.total_read_bytes,
                usage.total_written_bytes,
            )
        })
        .collect();
    sum_distinct_io(&named)
}

/// Sum cumulative (read, written) byte counters across disks, counting each
/// distinct disk *name* once. APFS synthetic volumes that share a physical
/// container report the same name ("Macintosh HD" for both `/` and
/// `/System/Volumes/Data`) and roughly the container's cumulative I/O, so
/// summing per volume double-counts; deduping by name collapses the twins
/// while genuinely separate disks (different names) still sum. Per-volume
/// counters can drift a few bytes under load, so an exact-counter key is
/// unreliable; the name is stable.
fn sum_distinct_io(disks: &[(String, u64, u64)]) -> (u64, u64) {
    let mut seen = HashSet::new();
    disks
        .iter()
        .fold((0u64, 0u64), |(r, w), (name, read, written)| {
            if seen.insert(name.as_str()) {
                (r.saturating_add(*read), w.saturating_add(*written))
            } else {
                (r, w)
            }
        })
}

/// Bytes/second between two cumulative byte counters over `dt_secs`.
/// Returns 0 on counter reset/wrap (now < prev) or non-positive dt.
fn bytes_per_sec(prev: u64, now: u64, dt_secs: f64) -> u64 {
    if dt_secs <= 0.0 {
        return 0;
    }
    if now < prev {
        return 0;
    }
    ((now - prev) as f64 / dt_secs) as u64
}

/// Current time in epoch milliseconds. Returns `0` if the system clock is before
/// the UNIX epoch instead of panicking.
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_freq_typical() {
        assert_eq!(max_freq_mhz(&[800, 4800, 2000]), Some(4800));
    }

    #[test]
    fn max_freq_empty_is_none() {
        assert_eq!(max_freq_mhz(&[]), None);
    }

    #[test]
    fn max_freq_all_zero_is_none() {
        assert_eq!(max_freq_mhz(&[0, 0]), None);
    }

    #[test]
    fn max_freq_mixed_with_zero() {
        assert_eq!(max_freq_mhz(&[0, 3000]), Some(3000));
    }

    #[test]
    fn bytes_per_sec_typical_rate() {
        // 2 MiB over 2 s -> 1 MiB/s.
        assert_eq!(bytes_per_sec(1_000_000, 3_097_152, 2.0), 1_048_576);
    }

    #[test]
    fn bytes_per_sec_reset_returns_zero() {
        assert_eq!(bytes_per_sec(5_000, 1_000, 1.0), 0);
    }

    #[test]
    fn bytes_per_sec_zero_dt_returns_zero() {
        assert_eq!(bytes_per_sec(1_000, 5_000, 0.0), 0);
    }

    #[test]
    fn bytes_per_sec_negative_dt_returns_zero() {
        assert_eq!(bytes_per_sec(1_000, 5_000, -1.0), 0);
    }

    #[test]
    fn bytes_per_sec_no_change_returns_zero() {
        assert_eq!(bytes_per_sec(4_096, 4_096, 1.0), 0);
    }

    fn named(rows: &[(&str, u64, u64)]) -> Vec<(String, u64, u64)> {
        rows.iter()
            .map(|(n, r, w)| (n.to_string(), *r, *w))
            .collect()
    }

    #[test]
    fn sum_distinct_io_collapses_same_name_twins() {
        // Same-name APFS twins collapse; first-seen wins, drifted counter ignored.
        assert_eq!(
            sum_distinct_io(&named(&[
                ("Macintosh HD", 100, 10),
                ("Macintosh HD", 95, 10)
            ])),
            (100, 10)
        );
    }

    #[test]
    fn sum_distinct_io_sums_distinct_names() {
        assert_eq!(
            sum_distinct_io(&named(&[("Macintosh HD", 100, 10), ("Docker", 50, 5)])),
            (150, 15)
        );
    }

    #[test]
    fn sum_distinct_io_empty_is_zero() {
        assert_eq!(sum_distinct_io(&[]), (0, 0));
    }

    #[test]
    fn sum_distinct_io_single_is_itself() {
        assert_eq!(sum_distinct_io(&named(&[("Macintosh HD", 7, 3)])), (7, 3));
    }

    #[test]
    fn sum_distinct_io_mixed_twins_and_distinct() {
        // Two "Macintosh HD" volumes collapse to the first; "Docker" and "Ext" add on.
        assert_eq!(
            sum_distinct_io(&named(&[
                ("Macintosh HD", 100, 10),
                ("Macintosh HD", 95, 10),
                ("Docker", 50, 5),
                ("Ext", 1, 1),
            ])),
            (151, 16)
        );
    }
}
