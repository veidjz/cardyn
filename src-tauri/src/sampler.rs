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

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sysinfo::{CpuRefreshKind, MemoryRefreshKind, System};
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
        Self {
            system,
            cpu_refresh,
            mem_refresh,
        }
    }

    /// Refresh CPU state and return a fresh [`MetricsSnapshot`].
    ///
    /// Does not sleep; the caller controls cadence. Values reflect the interval
    /// since the previous refresh (construction or the prior tick).
    pub fn tick(&mut self) -> MetricsSnapshot {
        self.system.refresh_cpu_specifics(self.cpu_refresh);
        self.system.refresh_memory_specifics(self.mem_refresh);

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
            ts_ms: now_ms(),
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
}
