//! Live CPU sampler probe.
//!
//! Builds a [`Sampler`], waits one update interval so the warm-up baseline ages,
//! then ticks a few times ~1 s apart and prints each snapshot. Proves the
//! sampler reads real, changing CPU values from the host.

use std::thread::sleep;
use std::time::Duration;

use cardyn_lib::sampler::Sampler;
use sysinfo::MINIMUM_CPU_UPDATE_INTERVAL;

fn main() {
    let mut sampler = Sampler::new();

    // Let the warm-up baseline age before the first real read.
    sleep(MINIMUM_CPU_UPDATE_INTERVAL);

    for i in 1..=3 {
        let s = sampler.tick();
        println!(
            "tick {i}: cpuTotal={:.1}% cores={} cpuFreqMhz={:?} tsMs={}",
            s.cpu_total,
            s.cpu_per_core.len(),
            s.cpu_freq_mhz,
            s.ts_ms,
        );
        println!(
            "         memUsed={} memTotal={} memAvailable={} memFree={} swapUsed={} swapTotal={}",
            s.mem_used, s.mem_total, s.mem_available, s.mem_free, s.swap_used, s.swap_total,
        );
        println!(
            "         diskUsed={} diskTotal={} diskReadBps={} diskWriteBps={}",
            s.disk_used, s.disk_total, s.disk_read_bps, s.disk_write_bps,
        );
        println!(
            "         netRxBps={} netTxBps={}",
            s.net_rx_bps, s.net_tx_bps,
        );
        if i < 3 {
            sleep(Duration::from_secs(1));
        }
    }
}
