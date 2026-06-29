//! GPU metrics provider.
//!
//! The sampler only ever sees the [`GpuProvider`] trait, so the native macOS
//! code (added in a later task) stays isolated and [`NoGpu`] is always a safe
//! fallback. Every reading is optional: an unavailable value is `None`
//! (shown as "GPU N/A"), never a fake `0`.

use serde::Serialize;

#[cfg(target_os = "macos")]
pub mod macos;

/// A single GPU reading. Each field is `None` when the underlying metric is
/// unavailable on this machine / OS. Serialized to camelCase JSON for the
/// frontend (`utilization`/`memUsed`/`vramTotal`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
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

    #[test]
    fn gpu_sample_serializes_camel_case() {
        let sample = GpuSample {
            utilization: Some(42.0),
            mem_used: Some(8_000_000),
            vram_total: Some(16_000_000_000),
        };
        let json = serde_json::to_string(&sample).expect("serialize");
        assert!(json.contains("\"utilization\""));
        assert!(json.contains("\"memUsed\""));
        assert!(json.contains("\"vramTotal\""));
        assert!(!json.contains("mem_used"));
        assert!(!json.contains("vram_total"));
    }

    #[test]
    fn gpu_sample_none_serializes_null() {
        let sample = GpuSample {
            utilization: None,
            mem_used: None,
            vram_total: None,
        };
        let json = serde_json::to_string(&sample).expect("serialize");
        assert!(json.contains("\"utilization\":null"));
        assert!(json.contains("\"memUsed\":null"));
        assert!(json.contains("\"vramTotal\":null"));
    }
}
