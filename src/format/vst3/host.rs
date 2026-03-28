//! VST3 host implementation

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

/// VST3 result codes
pub const K_RESULT_OK: i32 = 0;
pub const K_RESULT_TRUE: i32 = 0;
pub const K_RESULT_FALSE: i32 = 1;
pub const K_INVALID_ARGUMENT: i32 = 2;
pub const K_NOT_IMPLEMENTED: i32 = 3;
pub const K_INTERNAL_ERROR: i32 = 4;
pub const K_NOT_INITIALIZED: i32 = 5;
pub const K_OUT_OF_MEMORY: i32 = 6;

/// FUnknown interface - base for all VST3 interfaces
#[repr(C)]
pub struct FUnknown {
    pub vtable: *const FUnknownVtable,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FUnknownVtable {
    pub query_interface: unsafe extern "system" fn(this: *mut FUnknown, iid: *const [u8; 16], obj: *mut *mut c_void) -> i32,
    pub add_ref: unsafe extern "system" fn(this: *mut FUnknown) -> u32,
    pub release: unsafe extern "system" fn(this: *mut FUnknown) -> u32,
}

/// IPluginFactory interface
#[repr(C)]
pub struct IPluginFactory {
    pub unknown: FUnknown,
}

#[repr(C)]
pub struct IPluginFactoryVtable {
    pub unknown: FUnknownVtable,
    pub get_factory_info: unsafe extern "system" fn(this: *mut IPluginFactory, info: *mut PFactoryInfo) -> i32,
    pub count_classes: unsafe extern "system" fn(this: *mut IPluginFactory) -> i32,
    pub get_class_info: unsafe extern "system" fn(this: *mut IPluginFactory, index: i32, info: *mut PClassInfo) -> i32,
    pub create_instance: unsafe extern "system" fn(this: *mut IPluginFactory, cid: *const [u8; 16], iid: *const [u8; 16], obj: *mut *mut c_void) -> i32,
}

/// Factory info
#[repr(C)]
pub struct PFactoryInfo {
    pub vendor: [u8; 64],
    pub url: [u8; 256],
    pub email: [u8; 128],
    pub flags: i32,
}

/// Class info
#[repr(C)]
pub struct PClassInfo {
    pub cid: [u8; 16],
    pub cardinality: i32,
    pub category: [u8; 32],
    pub name: [u8; 64],
}

/// IComponent interface
#[repr(C)]
pub struct IComponent {
    pub unknown: FUnknown,
}

/// IAudioProcessor interface
#[repr(C)]
pub struct IAudioProcessor {
    pub unknown: FUnknown,
}

/// Process setup
#[repr(C)]
pub struct ProcessSetup {
    pub process_mode: i32,
    pub symbolic_sample_size: i32,
    pub max_samples_per_block: i32,
    pub sample_rate: f64,
}

/// Process data
#[repr(C)]
pub struct ProcessData {
    pub process_mode: i32,
    pub symbolic_sample_size: i32,
    pub num_samples: i32,
    pub num_inputs: i32,
    pub num_outputs: i32,
    pub inputs: *mut AudioBusBuffers,
    pub outputs: *mut AudioBusBuffers,
    pub input_param_changes: *mut c_void,
    pub output_param_changes: *mut c_void,
    pub input_events: *mut c_void,
    pub output_events: *mut c_void,
    pub context: *mut ProcessContext,
}

/// Audio bus buffers
#[repr(C)]
pub struct AudioBusBuffers {
    pub num_channels: i32,
    pub silence_flags: u64,
    pub channel_buffers32: *mut *mut f32,
    pub channel_buffers64: *mut *mut f64,
}

/// Process context (transport state)
#[repr(C)]
pub struct ProcessContext {
    pub state: u32,
    pub sample_rate: f64,
    pub project_time_samples: i64,
    pub system_time: i64,
    pub continuous_time_samples: i64,
    pub project_time_music: f64,
    pub bar_position_music: f64,
    pub cycle_start_music: f64,
    pub cycle_end_music: f64,
    pub tempo: f64,
    pub time_sig_numerator: i32,
    pub time_sig_denominator: i32,
    pub chord: i32,
    pub smpte_offset_subframes: i32,
    pub frame_rate: u32,
    pub samples_to_next_clock: i32,
}

/// VST3 host context
pub struct Vst3Host {
    ref_count: AtomicU32,
    pub name: String,
}

impl Vst3Host {
    pub fn new() -> Box<Self> {
        Box::new(Self {
            ref_count: AtomicU32::new(1),
            name: "Pulse".to_string(),
        })
    }

    pub fn add_ref(&self) -> u32 {
        self.ref_count.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn release(&self) -> u32 {
        self.ref_count.fetch_sub(1, Ordering::SeqCst) - 1
    }
}

impl Default for Vst3Host {
    fn default() -> Self {
        Self {
            ref_count: AtomicU32::new(1),
            name: "Pulse".to_string(),
        }
    }
}

/// Helper to convert VST3 result to Rust Result
pub fn check_result(result: i32) -> Result<(), &'static str> {
    match result {
        0 => Ok(()),
        1 => Err("False"),
        2 => Err("Invalid argument"),
        3 => Err("Not implemented"),
        4 => Err("Internal error"),
        5 => Err("Not initialized"),
        6 => Err("Out of memory"),
        _ => Err("Unknown error"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vst3_host_new() {
        let host = Vst3Host::new();
        assert_eq!(host.name, "Pulse");
    }

    #[test]
    fn test_vst3_result() {
        assert!(check_result(0).is_ok());
        assert!(check_result(1).is_err());
    }
}
