//! GPU metrics provider.
//!
//! The sampler only ever sees the [`GpuProvider`] trait, so the native macOS
//! code (added in a later task) stays isolated and [`NoGpu`] is always a safe
//! fallback. Every reading is optional: an unavailable value is `None`
//! (shown as "GPU N/A"), never a fake `0`.

/// A single GPU reading. Each field is `None` when the underlying metric is
/// unavailable on this machine / OS.
#[derive(Debug, Clone)]
pub struct GpuSample {
    /// GPU utilization, 0..=100 percent.
    pub utilization: Option<f32>,
    /// GPU memory in use, in bytes.
    pub mem_used: Option<u64>,
    /// Total VRAM in bytes. `None` on Apple Silicon (unified memory has no
    /// separate VRAM total).
    pub vram_total: Option<u64>,
}

/// A source of GPU samples. `&mut self` because a real provider may hold an
/// open OS handle that it reuses across ticks.
pub trait GpuProvider {
    fn sample(&mut self) -> GpuSample;
}

/// Fallback provider used when no GPU metrics are available. Always returns an
/// all-`None` sample.
pub struct NoGpu;

impl GpuProvider for NoGpu {
    fn sample(&mut self) -> GpuSample {
        GpuSample {
            utilization: None,
            mem_used: None,
            vram_total: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_gpu_sample_is_all_none() {
        let mut gpu = NoGpu;
        let s = gpu.sample();
        assert!(s.utilization.is_none());
        assert!(s.mem_used.is_none());
        assert!(s.vram_total.is_none());
    }
}
