//! macOS GPU provider backed by IOKit.
//!
//! Reads the `IOAccelerator` registry entry's `PerformanceStatistics`
//! sub-dictionary in-process. Every reading is optional: a missing key, a
//! wrong type, or no device yields `None` rather than a panic. We never
//! `unwrap`/`expect` on an FFI result.
//!
//! Apple Silicon uses unified memory, so there is no separate VRAM total and
//! [`GpuSample::vram_total`] is always `None`.

use std::ffi::CString;

use core_foundation::base::{kCFAllocatorDefault, CFType, TCFType};
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef, CFMutableDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use io_kit_sys::types::{io_iterator_t, io_object_t};
use io_kit_sys::{
    IOIteratorNext, IOObjectRelease, IORegistryEntryCreateCFProperties,
    IOServiceGetMatchingServices, IOServiceMatching,
};

use super::{GpuProvider, GpuSample};

/// IOKit class that exposes GPU performance counters.
const ACCELERATOR_CLASS: &str = "IOAccelerator";
/// Sub-dictionary inside the accelerator properties holding the live counters.
const PERFORMANCE_STATISTICS: &str = "PerformanceStatistics";
/// Counter key: GPU busy percentage.
const DEVICE_UTILIZATION: &str = "Device Utilization %";
/// Counter key: GPU memory currently in use, in bytes.
const IN_USE_SYSTEM_MEMORY: &str = "In use system memory";

/// `kern_return_t` success sentinel (`KERN_SUCCESS`). Inlined to avoid pulling
/// in `mach2` (a transitive-only dependency we cannot name directly).
const KERN_SUCCESS: i32 = 0;
/// `IO_OBJECT_NULL`: a null IOKit object / iterator.
const IO_OBJECT_NULL: io_object_t = 0;

/// GPU provider that reads metrics from macOS IOKit on each `sample()`.
pub struct MacGpu;

impl GpuProvider for MacGpu {
    fn sample(&mut self) -> GpuSample {
        query()
    }
}

/// An all-`None` sample, returned whenever any step of the query fails.
fn empty_sample() -> GpuSample {
    GpuSample {
        utilization: None,
        mem_used: None,
        vram_total: None,
    }
}

/// Walk the `IOAccelerator` services and return the first sample that yields a
/// counter. Any FFI failure degrades to an all-`None` sample; nothing panics.
fn query() -> GpuSample {
    let Ok(class) = CString::new(ACCELERATOR_CLASS) else {
        return empty_sample();
    };

    // SAFETY: `class` is a valid NUL-terminated C string that outlives the call.
    // `IOServiceMatching` returns a +1 (Create rule) matching dictionary, or
    // null if the class name is unknown.
    let matching = unsafe { IOServiceMatching(class.as_ptr()) };
    if matching.is_null() {
        return empty_sample();
    }

    let mut iterator: io_iterator_t = IO_OBJECT_NULL;
    // SAFETY: `matching` is a valid +1 matching dictionary. The call CONSUMES
    // that reference (so we must not release it ourselves) and, on success,
    // writes a live iterator into `iterator`. Port `0` == the default IOKit port.
    let kr = unsafe { IOServiceGetMatchingServices(0, matching as CFDictionaryRef, &mut iterator) };
    if kr != KERN_SUCCESS || iterator == IO_OBJECT_NULL {
        return empty_sample();
    }

    let mut out = empty_sample();
    loop {
        // SAFETY: `iterator` is a live io_iterator_t. Returns the next entry with
        // a +1 retain, or `IO_OBJECT_NULL` (0) once the iteration is exhausted.
        let entry = unsafe { IOIteratorNext(iterator) };
        if entry == IO_OBJECT_NULL {
            break;
        }

        let sample = read_entry(entry);

        // SAFETY: `entry` is a +1 io_object_t we own; release our reference.
        unsafe { IOObjectRelease(entry) };

        if let Some(sample) = sample {
            out = sample;
            break;
        }
    }

    // SAFETY: `iterator` is a +1 io_iterator_t we own; release our reference.
    unsafe { IOObjectRelease(iterator) };

    out
}

/// Read the `PerformanceStatistics` counters from a single registry entry.
/// Returns `None` when the entry has no usable counters.
fn read_entry(entry: io_object_t) -> Option<GpuSample> {
    let mut props_ref: CFMutableDictionaryRef = std::ptr::null_mut();
    // SAFETY: `entry` is a live io_registry_entry_t (alias of io_object_t). On
    // success the call writes a +1 (Create rule) properties dictionary into
    // `props_ref`; on failure it returns nonzero and leaves `props_ref` null.
    let kr =
        unsafe { IORegistryEntryCreateCFProperties(entry, &mut props_ref, kCFAllocatorDefault, 0) };
    if kr != KERN_SUCCESS || props_ref.is_null() {
        return None;
    }

    // SAFETY: `props_ref` is a non-null, +1 CFDictionaryRef. `wrap_under_create_rule`
    // takes ownership, so the resulting value's Drop performs the single matching
    // CFRelease (no manual release / no double free).
    let props: CFDictionary<CFString, CFType> =
        unsafe { CFDictionary::wrap_under_create_rule(props_ref as CFDictionaryRef) };

    // `downcast` checks the CF type id and returns `None` on a wrong type, so a
    // malformed dictionary degrades gracefully instead of misinterpreting bytes.
    let perf = props
        .find(CFString::new(PERFORMANCE_STATISTICS))?
        .downcast::<CFDictionary>()?;

    // SAFETY: `perf` is a valid, non-null CFDictionaryRef that `downcast` already
    // type-checked and retained; here we only re-type its generic parameters so
    // we can look keys up by `CFString`. `wrap_under_get_rule` adds its own
    // retain balanced by Drop.
    let perf: CFDictionary<CFString, CFType> =
        unsafe { CFDictionary::wrap_under_get_rule(perf.as_concrete_TypeRef()) };

    let utilization = cf_number(&perf, DEVICE_UTILIZATION).and_then(|n| n.to_f32());
    let mem_used = cf_number(&perf, IN_USE_SYSTEM_MEMORY)
        .and_then(|n| n.to_i64())
        .and_then(|bytes| u64::try_from(bytes).ok());

    if utilization.is_none() && mem_used.is_none() {
        return None;
    }

    Some(GpuSample {
        utilization,
        mem_used,
        // Unified memory on Apple Silicon: no separate VRAM total.
        vram_total: None,
    })
}

/// Look up `key` in `dict` and return it as a [`CFNumber`], or `None` if the key
/// is missing or the value is not a number.
fn cf_number(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<CFNumber> {
    dict.find(CFString::new(key))?.downcast::<CFNumber>()
}
