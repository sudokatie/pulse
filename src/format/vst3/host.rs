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
#[derive(Clone, Copy)]
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

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IComponentVtable {
    pub unknown: FUnknownVtable,
    // IPluginBase
    pub initialize: unsafe extern "system" fn(this: *mut IComponent, context: *mut c_void) -> i32,
    pub terminate: unsafe extern "system" fn(this: *mut IComponent) -> i32,
    // IComponent
    pub get_controller_class_id: unsafe extern "system" fn(this: *mut IComponent, class_id: *mut [u8; 16]) -> i32,
    pub set_io_mode: unsafe extern "system" fn(this: *mut IComponent, mode: i32) -> i32,
    pub get_bus_count: unsafe extern "system" fn(this: *mut IComponent, media_type: i32, dir: i32) -> i32,
    pub get_bus_info: unsafe extern "system" fn(this: *mut IComponent, media_type: i32, dir: i32, index: i32, info: *mut BusInfo) -> i32,
    pub get_routing_info: unsafe extern "system" fn(this: *mut IComponent, in_info: *mut RoutingInfo, out_info: *mut RoutingInfo) -> i32,
    pub activate_bus: unsafe extern "system" fn(this: *mut IComponent, media_type: i32, dir: i32, index: i32, state: u8) -> i32,
    pub set_active: unsafe extern "system" fn(this: *mut IComponent, state: u8) -> i32,
    pub set_state: unsafe extern "system" fn(this: *mut IComponent, state: *mut IBStream) -> i32,
    pub get_state: unsafe extern "system" fn(this: *mut IComponent, state: *mut IBStream) -> i32,
}

/// Bus info
#[repr(C)]
pub struct BusInfo {
    pub media_type: i32,
    pub direction: i32,
    pub channel_count: i32,
    pub name: [u16; 128],
    pub bus_type: i32,
    pub flags: u32,
}

/// Routing info
#[repr(C)]
pub struct RoutingInfo {
    pub media_type: i32,
    pub bus_index: i32,
    pub channel: i32,
}

/// IBStream interface for state
#[repr(C)]
pub struct IBStream {
    pub unknown: FUnknown,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IBStreamVtable {
    pub unknown: FUnknownVtable,
    pub read: unsafe extern "system" fn(this: *mut IBStream, buffer: *mut c_void, num_bytes: i32, num_bytes_read: *mut i32) -> i32,
    pub write: unsafe extern "system" fn(this: *mut IBStream, buffer: *const c_void, num_bytes: i32, num_bytes_written: *mut i32) -> i32,
    pub seek: unsafe extern "system" fn(this: *mut IBStream, pos: i64, mode: i32, result: *mut i64) -> i32,
    pub tell: unsafe extern "system" fn(this: *mut IBStream, pos: *mut i64) -> i32,
}

/// IAudioProcessor interface
#[repr(C)]
pub struct IAudioProcessor {
    pub unknown: FUnknown,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IAudioProcessorVtable {
    pub unknown: FUnknownVtable,
    pub set_bus_arrangements: unsafe extern "system" fn(this: *mut IAudioProcessor, inputs: *const u64, num_ins: i32, outputs: *const u64, num_outs: i32) -> i32,
    pub get_bus_arrangement: unsafe extern "system" fn(this: *mut IAudioProcessor, dir: i32, index: i32, arr: *mut u64) -> i32,
    pub can_process_sample_size: unsafe extern "system" fn(this: *mut IAudioProcessor, symbolic_sample_size: i32) -> i32,
    pub get_latency_samples: unsafe extern "system" fn(this: *mut IAudioProcessor) -> u32,
    pub setup_processing: unsafe extern "system" fn(this: *mut IAudioProcessor, setup: *const ProcessSetup) -> i32,
    pub set_processing: unsafe extern "system" fn(this: *mut IAudioProcessor, state: u8) -> i32,
    pub process: unsafe extern "system" fn(this: *mut IAudioProcessor, data: *mut ProcessData) -> i32,
    pub get_tail_samples: unsafe extern "system" fn(this: *mut IAudioProcessor) -> u32,
}

/// IEditController interface for parameters
#[repr(C)]
pub struct IEditController {
    pub unknown: FUnknown,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IEditControllerVtable {
    pub unknown: FUnknownVtable,
    // IPluginBase
    pub initialize: unsafe extern "system" fn(this: *mut IEditController, context: *mut c_void) -> i32,
    pub terminate: unsafe extern "system" fn(this: *mut IEditController) -> i32,
    // IEditController
    pub set_component_state: unsafe extern "system" fn(this: *mut IEditController, state: *mut IBStream) -> i32,
    pub set_state: unsafe extern "system" fn(this: *mut IEditController, state: *mut IBStream) -> i32,
    pub get_state: unsafe extern "system" fn(this: *mut IEditController, state: *mut IBStream) -> i32,
    pub get_parameter_count: unsafe extern "system" fn(this: *mut IEditController) -> i32,
    pub get_parameter_info: unsafe extern "system" fn(this: *mut IEditController, param_index: i32, info: *mut ParameterInfo) -> i32,
    pub get_param_string_by_value: unsafe extern "system" fn(this: *mut IEditController, id: u32, value_normalized: f64, string: *mut [u16; 128]) -> i32,
    pub get_param_value_by_string: unsafe extern "system" fn(this: *mut IEditController, id: u32, string: *const u16, value_normalized: *mut f64) -> i32,
    pub normalized_param_to_plain: unsafe extern "system" fn(this: *mut IEditController, id: u32, value_normalized: f64) -> f64,
    pub plain_param_to_normalized: unsafe extern "system" fn(this: *mut IEditController, id: u32, plain_value: f64) -> f64,
    pub get_param_normalized: unsafe extern "system" fn(this: *mut IEditController, id: u32) -> f64,
    pub set_param_normalized: unsafe extern "system" fn(this: *mut IEditController, id: u32, value: f64) -> i32,
    pub set_component_handler: unsafe extern "system" fn(this: *mut IEditController, handler: *mut c_void) -> i32,
    pub create_view: unsafe extern "system" fn(this: *mut IEditController, name: *const c_void) -> *mut c_void,
}

/// Parameter info
#[repr(C)]
pub struct ParameterInfo {
    pub id: u32,
    pub title: [u16; 128],
    pub short_title: [u16; 128],
    pub units: [u16; 128],
    pub step_count: i32,
    pub default_normalized_value: f64,
    pub unit_id: i32,
    pub flags: i32,
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

/// Memory stream for state handling
pub struct MemoryStream {
    data: Vec<u8>,
    position: usize,
}

impl MemoryStream {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            position: 0,
        }
    }

    pub fn from_data(data: Vec<u8>) -> Self {
        Self { data, position: 0 }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> usize {
        let available = self.data.len() - self.position;
        let to_read = buffer.len().min(available);
        buffer[..to_read].copy_from_slice(&self.data[self.position..self.position + to_read]);
        self.position += to_read;
        to_read
    }

    pub fn write(&mut self, buffer: &[u8]) -> usize {
        self.data.extend_from_slice(buffer);
        self.position = self.data.len();
        buffer.len()
    }

    pub fn seek(&mut self, pos: i64, mode: i32) -> i64 {
        let new_pos = match mode {
            0 => pos as usize,                              // Seek set
            1 => (self.position as i64 + pos) as usize,     // Seek cur
            2 => (self.data.len() as i64 + pos) as usize,   // Seek end
            _ => self.position,
        };
        self.position = new_pos.min(self.data.len());
        self.position as i64
    }

    pub fn tell(&self) -> i64 {
        self.position as i64
    }
}

impl Default for MemoryStream {
    fn default() -> Self {
        Self::new()
    }
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

    #[test]
    fn test_memory_stream() {
        let mut stream = MemoryStream::new();
        let written = stream.write(b"hello");
        assert_eq!(written, 5);
        
        stream.seek(0, 0);
        let mut buf = [0u8; 5];
        let read = stream.read(&mut buf);
        assert_eq!(read, 5);
        assert_eq!(&buf, b"hello");
    }

    #[test]
    fn test_memory_stream_seek() {
        let mut stream = MemoryStream::from_data(b"hello world".to_vec());
        
        stream.seek(6, 0); // seek set
        assert_eq!(stream.tell(), 6);
        
        stream.seek(2, 1); // seek cur
        assert_eq!(stream.tell(), 8);
        
        stream.seek(-3, 2); // seek end
        assert_eq!(stream.tell(), 8);
    }
}
