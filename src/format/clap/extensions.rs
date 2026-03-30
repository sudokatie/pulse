//! CLAP extension support

use std::ffi::{c_char, c_void, CStr};
use std::ptr;

/// CLAP extension IDs
pub mod ext_id {
    pub const PARAMS: &[u8] = b"clap.params\0";
    pub const STATE: &[u8] = b"clap.state\0";
    pub const AUDIO_PORTS: &[u8] = b"clap.audio-ports\0";
    pub const NOTE_PORTS: &[u8] = b"clap.note-ports\0";
    pub const LATENCY: &[u8] = b"clap.latency\0";
    pub const TAIL: &[u8] = b"clap.tail\0";
    pub const GUI: &[u8] = b"clap.gui\0";
    pub const TIMER_SUPPORT: &[u8] = b"clap.timer-support\0";
}

/// Parameter flags
pub mod param_flags {
    pub const IS_STEPPED: u32 = 1 << 0;
    pub const IS_PERIODIC: u32 = 1 << 1;
    pub const IS_HIDDEN: u32 = 1 << 2;
    pub const IS_READONLY: u32 = 1 << 3;
    pub const IS_BYPASS: u32 = 1 << 4;
    pub const IS_AUTOMATABLE: u32 = 1 << 5;
    pub const IS_AUTOMATABLE_PER_NOTE_ID: u32 = 1 << 6;
    pub const IS_AUTOMATABLE_PER_KEY: u32 = 1 << 7;
    pub const IS_AUTOMATABLE_PER_CHANNEL: u32 = 1 << 8;
    pub const IS_AUTOMATABLE_PER_PORT: u32 = 1 << 9;
    pub const IS_MODULATABLE: u32 = 1 << 10;
    pub const IS_MODULATABLE_PER_NOTE_ID: u32 = 1 << 11;
    pub const IS_MODULATABLE_PER_KEY: u32 = 1 << 12;
    pub const IS_MODULATABLE_PER_CHANNEL: u32 = 1 << 13;
    pub const IS_MODULATABLE_PER_PORT: u32 = 1 << 14;
    pub const REQUIRES_PROCESS: u32 = 1 << 15;
}

/// Parameter info from CLAP params extension
#[repr(C)]
pub struct ClapParamInfo {
    pub id: u32,
    pub flags: u32,
    pub cookie: *mut c_void,
    pub name: [c_char; 256],
    pub module: [c_char; 1024],
    pub min_value: f64,
    pub max_value: f64,
    pub default_value: f64,
}

impl Default for ClapParamInfo {
    fn default() -> Self {
        Self {
            id: 0,
            flags: 0,
            cookie: ptr::null_mut(),
            name: [0; 256],
            module: [0; 1024],
            min_value: 0.0,
            max_value: 1.0,
            default_value: 0.0,
        }
    }
}

impl ClapParamInfo {
    /// Get parameter name as string
    pub fn name_str(&self) -> String {
        unsafe {
            CStr::from_ptr(self.name.as_ptr())
                .to_string_lossy()
                .to_string()
        }
    }

    /// Get module path as string
    pub fn module_str(&self) -> String {
        unsafe {
            CStr::from_ptr(self.module.as_ptr())
                .to_string_lossy()
                .to_string()
        }
    }

    /// Check if parameter is automatable
    pub fn is_automatable(&self) -> bool {
        self.flags & param_flags::IS_AUTOMATABLE != 0
    }

    /// Check if parameter is modulatable
    pub fn is_modulatable(&self) -> bool {
        self.flags & param_flags::IS_MODULATABLE != 0
    }

    /// Check if parameter is stepped (discrete)
    pub fn is_stepped(&self) -> bool {
        self.flags & param_flags::IS_STEPPED != 0
    }

    /// Check if parameter is hidden
    pub fn is_hidden(&self) -> bool {
        self.flags & param_flags::IS_HIDDEN != 0
    }
}

/// CLAP params extension interface
#[repr(C)]
pub struct ClapPluginParams {
    pub count: Option<unsafe extern "C" fn(plugin: *const c_void) -> u32>,
    pub get_info: Option<unsafe extern "C" fn(plugin: *const c_void, param_index: u32, param_info: *mut ClapParamInfo) -> bool>,
    pub get_value: Option<unsafe extern "C" fn(plugin: *const c_void, param_id: u32, out_value: *mut f64) -> bool>,
    pub value_to_text: Option<unsafe extern "C" fn(plugin: *const c_void, param_id: u32, value: f64, out_buffer: *mut c_char, out_buffer_capacity: u32) -> bool>,
    pub text_to_value: Option<unsafe extern "C" fn(plugin: *const c_void, param_id: u32, param_value_text: *const c_char, out_value: *mut f64) -> bool>,
    pub flush: Option<unsafe extern "C" fn(plugin: *const c_void, in_events: *const c_void, out_events: *const c_void)>,
}

/// CLAP state extension interface
#[repr(C)]
pub struct ClapPluginState {
    pub save: Option<unsafe extern "C" fn(plugin: *const c_void, stream: *const ClapOstream) -> bool>,
    pub load: Option<unsafe extern "C" fn(plugin: *const c_void, stream: *const ClapIstream) -> bool>,
}

/// CLAP output stream for saving state
#[repr(C)]
pub struct ClapOstream {
    pub ctx: *mut c_void,
    pub write: Option<unsafe extern "C" fn(stream: *const ClapOstream, buffer: *const c_void, size: u64) -> i64>,
}

/// CLAP input stream for loading state
#[repr(C)]
pub struct ClapIstream {
    pub ctx: *mut c_void,
    pub read: Option<unsafe extern "C" fn(stream: *const ClapIstream, buffer: *mut c_void, size: u64) -> i64>,
}

/// Audio port type
#[repr(C)]
pub struct ClapAudioPortInfo {
    pub id: u32,
    pub name: [c_char; 256],
    pub flags: u32,
    pub channel_count: u32,
    pub port_type: *const c_char,
    pub in_place_pair: u32,
}

/// CLAP audio-ports extension interface
#[repr(C)]
pub struct ClapPluginAudioPorts {
    pub count: Option<unsafe extern "C" fn(plugin: *const c_void, is_input: bool) -> u32>,
    pub get: Option<unsafe extern "C" fn(plugin: *const c_void, index: u32, is_input: bool, info: *mut ClapAudioPortInfo) -> bool>,
}

/// Note port info
#[repr(C)]
pub struct ClapNotePortInfo {
    pub id: u32,
    pub supported_dialects: u32,
    pub preferred_dialect: u32,
    pub name: [c_char; 256],
}

/// CLAP note-ports extension interface
#[repr(C)]
pub struct ClapPluginNotePorts {
    pub count: Option<unsafe extern "C" fn(plugin: *const c_void, is_input: bool) -> u32>,
    pub get: Option<unsafe extern "C" fn(plugin: *const c_void, index: u32, is_input: bool, info: *mut ClapNotePortInfo) -> bool>,
}

/// CLAP latency extension interface
#[repr(C)]
pub struct ClapPluginLatency {
    pub get: Option<unsafe extern "C" fn(plugin: *const c_void) -> u32>,
}

/// CLAP tail extension interface
#[repr(C)]
pub struct ClapPluginTail {
    pub get: Option<unsafe extern "C" fn(plugin: *const c_void) -> u32>,
}

/// Memory buffer for state serialization
pub struct StateBuffer {
    data: Vec<u8>,
    position: usize,
}

impl StateBuffer {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            position: 0,
        }
    }

    pub fn from_data(data: Vec<u8>) -> Self {
        Self { data, position: 0 }
    }

    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn write(&mut self, buffer: &[u8]) -> i64 {
        self.data.extend_from_slice(buffer);
        buffer.len() as i64
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> i64 {
        let remaining = self.data.len() - self.position;
        let to_read = buffer.len().min(remaining);
        buffer[..to_read].copy_from_slice(&self.data[self.position..self.position + to_read]);
        self.position += to_read;
        to_read as i64
    }
}

impl Default for StateBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create an output stream wrapping a StateBuffer
pub fn create_ostream(buffer: &mut StateBuffer) -> ClapOstream {
    ClapOstream {
        ctx: buffer as *mut StateBuffer as *mut c_void,
        write: Some(ostream_write),
    }
}

/// Helper to create an input stream wrapping a StateBuffer
pub fn create_istream(buffer: &mut StateBuffer) -> ClapIstream {
    ClapIstream {
        ctx: buffer as *mut StateBuffer as *mut c_void,
        read: Some(istream_read),
    }
}

unsafe extern "C" fn ostream_write(stream: *const ClapOstream, buffer: *const c_void, size: u64) -> i64 {
    let state_buffer = &mut *((*stream).ctx as *mut StateBuffer);
    let slice = std::slice::from_raw_parts(buffer as *const u8, size as usize);
    state_buffer.write(slice)
}

unsafe extern "C" fn istream_read(stream: *const ClapIstream, buffer: *mut c_void, size: u64) -> i64 {
    let state_buffer = &mut *((*stream).ctx as *mut StateBuffer);
    let slice = std::slice::from_raw_parts_mut(buffer as *mut u8, size as usize);
    state_buffer.read(slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_param_info_default() {
        let info = ClapParamInfo::default();
        assert_eq!(info.id, 0);
        assert_eq!(info.min_value, 0.0);
        assert_eq!(info.max_value, 1.0);
    }

    #[test]
    fn test_param_flags() {
        let flags = param_flags::IS_AUTOMATABLE | param_flags::IS_MODULATABLE;
        let info = ClapParamInfo {
            flags,
            ..Default::default()
        };
        assert!(info.is_automatable());
        assert!(info.is_modulatable());
        assert!(!info.is_stepped());
        assert!(!info.is_hidden());
    }

    #[test]
    fn test_state_buffer_write_read() {
        let mut buffer = StateBuffer::new();
        buffer.write(b"hello world");
        
        let mut read_buf = [0u8; 5];
        let read = buffer.read(&mut read_buf);
        assert_eq!(read, 5);
        assert_eq!(&read_buf, b"hello");
        
        let read = buffer.read(&mut read_buf);
        assert_eq!(read, 5);
        assert_eq!(&read_buf, b" worl");
    }

    #[test]
    fn test_state_buffer_into_data() {
        let mut buffer = StateBuffer::new();
        buffer.write(b"test data");
        let data = buffer.into_data();
        assert_eq!(data, b"test data");
    }
}
