//! VST3 IAudioProcessor interface definition

use super::com::IUnknownVtable;
use super::types::{TResult, ProcessSetup, ProcessData};

/// IAudioProcessor interface
#[repr(C)]
pub struct IAudioProcessor {
    pub vtable: *const IAudioProcessorVtable,
}

/// IAudioProcessor vtable
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IAudioProcessorVtable {
    // IUnknown
    pub unknown: IUnknownVtable,
    // IAudioProcessor
    pub set_bus_arrangements: unsafe extern "system" fn(
        this: *mut IAudioProcessor,
        inputs: *const u64,
        num_ins: i32,
        outputs: *const u64,
        num_outs: i32,
    ) -> TResult,
    pub get_bus_arrangement: unsafe extern "system" fn(
        this: *mut IAudioProcessor,
        dir: i32,
        index: i32,
        arr: *mut u64,
    ) -> TResult,
    pub can_process_sample_size: unsafe extern "system" fn(
        this: *mut IAudioProcessor,
        symbolic_sample_size: i32,
    ) -> TResult,
    pub get_latency_samples: unsafe extern "system" fn(
        this: *mut IAudioProcessor,
    ) -> u32,
    pub setup_processing: unsafe extern "system" fn(
        this: *mut IAudioProcessor,
        setup: *const ProcessSetup,
    ) -> TResult,
    pub set_processing: unsafe extern "system" fn(
        this: *mut IAudioProcessor,
        state: u8,
    ) -> TResult,
    pub process: unsafe extern "system" fn(
        this: *mut IAudioProcessor,
        data: *mut ProcessData,
    ) -> TResult,
    pub get_tail_samples: unsafe extern "system" fn(
        this: *mut IAudioProcessor,
    ) -> u32,
}

/// Symbolic sample size constants
pub const K_SAMPLE_32: i32 = 0;
pub const K_SAMPLE_64: i32 = 1;

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::c_void;

    #[test]
    fn test_iaudio_processor_vtable_layout() {
        // Verify vtable is pointer-aligned
        assert_eq!(
            std::mem::align_of::<IAudioProcessorVtable>(),
            std::mem::align_of::<*const c_void>()
        );

        // IAudioProcessorVtable should have:
        // - 3 pointers from IUnknown
        // - 8 pointers from IAudioProcessor
        // Total: 11 pointers
        let expected_size = 11 * std::mem::size_of::<*const c_void>();
        assert_eq!(std::mem::size_of::<IAudioProcessorVtable>(), expected_size);
    }

    #[test]
    fn test_iaudio_processor_inherits_iunknown() {
        // Verify that the first field is IUnknownVtable
        let vtable_offset = std::mem::offset_of!(IAudioProcessorVtable, unknown);
        assert_eq!(vtable_offset, 0);
    }

    #[test]
    fn test_sample_size_constants() {
        assert_eq!(K_SAMPLE_32, 0);
        assert_eq!(K_SAMPLE_64, 1);
    }
}
