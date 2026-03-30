//! CLAP plugin instance with full extension support

use super::extensions::*;
use super::host::{ClapHost, ClapVersion, HostData};
use crate::buffer::AudioBuffer;
use crate::plugin::{Plugin, PluginCategory, PluginConfig, PluginInfo};
use crate::process::ProcessContext;
use crate::{Error, Result};
use libloading::{Library, Symbol};
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::Path;
use std::ptr;

// CLAP C ABI types
#[repr(C)]
struct ClapPluginEntry {
    clap_version: ClapVersion,
    init: Option<unsafe extern "C" fn(plugin_path: *const c_char) -> bool>,
    deinit: Option<unsafe extern "C" fn()>,
    get_factory: Option<unsafe extern "C" fn(factory_id: *const c_char) -> *const c_void>,
}

#[repr(C)]
struct ClapPluginFactory {
    get_plugin_count: Option<unsafe extern "C" fn(factory: *const ClapPluginFactory) -> u32>,
    get_plugin_descriptor: Option<unsafe extern "C" fn(factory: *const ClapPluginFactory, index: u32) -> *const ClapPluginDescriptor>,
    create_plugin: Option<unsafe extern "C" fn(factory: *const ClapPluginFactory, host: *const ClapHost, plugin_id: *const c_char) -> *const ClapPlugin>,
}

#[repr(C)]
struct ClapPluginDescriptor {
    clap_version: ClapVersion,
    id: *const c_char,
    name: *const c_char,
    vendor: *const c_char,
    url: *const c_char,
    manual_url: *const c_char,
    support_url: *const c_char,
    version: *const c_char,
    description: *const c_char,
    features: *const *const c_char,
}

#[repr(C)]
struct ClapPlugin {
    desc: *const ClapPluginDescriptor,
    plugin_data: *mut c_void,
    init: Option<unsafe extern "C" fn(plugin: *const ClapPlugin) -> bool>,
    destroy: Option<unsafe extern "C" fn(plugin: *const ClapPlugin)>,
    activate: Option<unsafe extern "C" fn(plugin: *const ClapPlugin, sample_rate: f64, min_frames: u32, max_frames: u32) -> bool>,
    deactivate: Option<unsafe extern "C" fn(plugin: *const ClapPlugin)>,
    start_processing: Option<unsafe extern "C" fn(plugin: *const ClapPlugin) -> bool>,
    stop_processing: Option<unsafe extern "C" fn(plugin: *const ClapPlugin)>,
    reset: Option<unsafe extern "C" fn(plugin: *const ClapPlugin)>,
    process: Option<unsafe extern "C" fn(plugin: *const ClapPlugin, process: *const ClapProcess) -> i32>,
    get_extension: Option<unsafe extern "C" fn(plugin: *const ClapPlugin, id: *const c_char) -> *const c_void>,
    on_main_thread: Option<unsafe extern "C" fn(plugin: *const ClapPlugin)>,
}

#[repr(C)]
struct ClapProcess {
    steady_time: i64,
    frames_count: u32,
    transport: *const ClapEventTransport,
    audio_inputs: *const ClapAudioBuffer,
    audio_outputs: *mut ClapAudioBuffer,
    audio_inputs_count: u32,
    audio_outputs_count: u32,
    in_events: *const ClapInputEvents,
    out_events: *const ClapOutputEvents,
}

#[repr(C)]
struct ClapAudioBuffer {
    data32: *mut *mut f32,
    data64: *mut *mut f64,
    channel_count: u32,
    latency: u32,
    constant_mask: u64,
}

#[repr(C)]
struct ClapEventTransport {
    header: ClapEventHeader,
    flags: u32,
    song_pos_beats: i64,
    song_pos_seconds: i64,
    tempo: f64,
    tempo_inc: f64,
    loop_start_beats: i64,
    loop_end_beats: i64,
    loop_start_seconds: i64,
    loop_end_seconds: i64,
    bar_start: i64,
    bar_number: i32,
    tsig_num: u16,
    tsig_denom: u16,
}

#[repr(C)]
struct ClapEventHeader {
    size: u32,
    time: u32,
    space_id: u16,
    type_: u16,
    flags: u32,
}

#[repr(C)]
struct ClapInputEvents {
    ctx: *mut c_void,
    size: Option<unsafe extern "C" fn(list: *const ClapInputEvents) -> u32>,
    get: Option<unsafe extern "C" fn(list: *const ClapInputEvents, index: u32) -> *const ClapEventHeader>,
}

#[repr(C)]
struct ClapOutputEvents {
    ctx: *mut c_void,
    try_push: Option<unsafe extern "C" fn(list: *const ClapOutputEvents, event: *const ClapEventHeader) -> bool>,
}

const CLAP_PLUGIN_FACTORY_ID: &[u8] = b"clap.plugin-factory\0";

/// CLAP plugin instance with extension support
pub struct ClapInstance {
    _library: Library,
    plugin: *const ClapPlugin,
    _host: Box<ClapHost>,
    info: PluginInfo,
    sample_rate: f32,
    activated: bool,
    processing: bool,
    // Extensions
    params_ext: *const ClapPluginParams,
    state_ext: *const ClapPluginState,
    latency_ext: *const ClapPluginLatency,
    tail_ext: *const ClapPluginTail,
    audio_ports_ext: *const ClapPluginAudioPorts,
    note_ports_ext: *const ClapPluginNotePorts,
    // Cached extension data
    param_count: u32,
    // Audio buffers
    input_ptrs: Vec<*mut f32>,
    output_ptrs: Vec<*mut f32>,
    input_buffer: Vec<Vec<f32>>,
    output_buffer: Vec<Vec<f32>>,
}

unsafe impl Send for ClapInstance {}
unsafe impl Sync for ClapInstance {}

impl ClapInstance {
    /// Load a CLAP plugin from a path
    pub fn load(path: &Path, plugin_id: &str) -> Result<Self> {
        let binary_path = if cfg!(target_os = "macos") {
            path.join("Contents").join("MacOS").join(
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("plugin")
            )
        } else if cfg!(target_os = "windows") {
            path.join(
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| format!("{}.dll", s))
                    .unwrap_or_else(|| "plugin.dll".to_string())
            )
        } else {
            let so_name = path.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| format!("{}.so", s))
                .unwrap_or_else(|| "plugin.so".to_string());
            if path.is_dir() {
                path.join(&so_name)
            } else {
                path.to_path_buf()
            }
        };

        let library = unsafe {
            Library::new(&binary_path)
                .map_err(|e| Error::Plugin(format!("Failed to load CLAP plugin: {}", e)))?
        };

        let entry: Symbol<*const ClapPluginEntry> = unsafe {
            library.get(b"clap_entry")
                .map_err(|e| Error::Plugin(format!("Failed to find clap_entry: {}", e)))?
        };

        let entry = unsafe { &**entry };

        let path_cstr = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| Error::Plugin("Invalid path".into()))?;
        
        let init_ok = unsafe {
            entry.init.map(|f| f(path_cstr.as_ptr())).unwrap_or(true)
        };
        
        if !init_ok {
            return Err(Error::Plugin("Plugin init failed".into()));
        }

        let factory_id = CLAP_PLUGIN_FACTORY_ID.as_ptr() as *const c_char;
        let factory = unsafe {
            entry.get_factory.map(|f| f(factory_id))
                .ok_or_else(|| Error::Plugin("No factory".into()))?
        } as *const ClapPluginFactory;

        if factory.is_null() {
            return Err(Error::Plugin("Null factory".into()));
        }

        let plugin_id_cstr = CString::new(plugin_id)
            .map_err(|_| Error::Plugin("Invalid plugin ID".into()))?;

        let host_data = HostData::new();
        let host = ClapHost::new(host_data);
        let host_ptr = &*host as *const ClapHost;

        let plugin = unsafe {
            let factory_ref = &*factory;
            factory_ref.create_plugin
                .map(|f| f(factory, host_ptr, plugin_id_cstr.as_ptr()))
                .ok_or_else(|| Error::Plugin("No create_plugin".into()))?
        };

        if plugin.is_null() {
            return Err(Error::Plugin("Failed to create plugin".into()));
        }

        // Get plugin info from descriptor
        let info = unsafe {
            let plugin_ref = &*plugin;
            if plugin_ref.desc.is_null() {
                return Err(Error::Plugin("No descriptor".into()));
            }
            let desc = &*plugin_ref.desc;
            
            let name = if desc.name.is_null() {
                "Unknown".to_string()
            } else {
                CStr::from_ptr(desc.name).to_string_lossy().to_string()
            };
            
            let vendor = if desc.vendor.is_null() {
                "Unknown".to_string()
            } else {
                CStr::from_ptr(desc.vendor).to_string_lossy().to_string()
            };
            
            let version = if desc.version.is_null() {
                "1.0.0".to_string()
            } else {
                CStr::from_ptr(desc.version).to_string_lossy().to_string()
            };

            PluginInfo {
                id: plugin_id.to_string(),
                name,
                vendor,
                version,
                category: PluginCategory::Effect,
                inputs: 2,
                outputs: 2,
            }
        };

        // Initialize the plugin
        unsafe {
            let plugin_ref = &*plugin;
            if let Some(init) = plugin_ref.init {
                if !init(plugin) {
                    return Err(Error::Plugin("Plugin init failed".into()));
                }
            }
        }

        // Query extensions
        let (params_ext, state_ext, latency_ext, tail_ext, audio_ports_ext, note_ports_ext) = unsafe {
            let plugin_ref = &*plugin;
            let get_ext = plugin_ref.get_extension.unwrap_or(null_get_extension);
            
            let params = get_ext(plugin, ext_id::PARAMS.as_ptr() as *const c_char) as *const ClapPluginParams;
            let state = get_ext(plugin, ext_id::STATE.as_ptr() as *const c_char) as *const ClapPluginState;
            let latency = get_ext(plugin, ext_id::LATENCY.as_ptr() as *const c_char) as *const ClapPluginLatency;
            let tail = get_ext(plugin, ext_id::TAIL.as_ptr() as *const c_char) as *const ClapPluginTail;
            let audio_ports = get_ext(plugin, ext_id::AUDIO_PORTS.as_ptr() as *const c_char) as *const ClapPluginAudioPorts;
            let note_ports = get_ext(plugin, ext_id::NOTE_PORTS.as_ptr() as *const c_char) as *const ClapPluginNotePorts;
            
            (params, state, latency, tail, audio_ports, note_ports)
        };

        // Get parameter count
        let param_count = if !params_ext.is_null() {
            unsafe {
                (*params_ext).count.map(|f| f(plugin as *const c_void)).unwrap_or(0)
            }
        } else {
            0
        };

        Ok(Self {
            _library: library,
            plugin,
            _host: host,
            info,
            sample_rate: 44100.0,
            activated: false,
            processing: false,
            params_ext,
            state_ext,
            latency_ext,
            tail_ext,
            audio_ports_ext,
            note_ports_ext,
            param_count,
            input_ptrs: vec![ptr::null_mut(); 2],
            output_ptrs: vec![ptr::null_mut(); 2],
            input_buffer: vec![vec![0.0; 4096]; 2],
            output_buffer: vec![vec![0.0; 4096]; 2],
        })
    }

    /// Get parameter count
    pub fn parameter_count(&self) -> u32 {
        self.param_count
    }

    /// Get parameter info
    pub fn parameter_info(&self, index: u32) -> Option<ClapParamInfo> {
        if self.params_ext.is_null() || index >= self.param_count {
            return None;
        }

        unsafe {
            let ext = &*self.params_ext;
            let get_info = ext.get_info?;
            let mut info = ClapParamInfo::default();
            if get_info(self.plugin as *const c_void, index, &mut info) {
                Some(info)
            } else {
                None
            }
        }
    }

    /// Get parameter value by ID
    pub fn get_param_value(&self, param_id: u32) -> Option<f64> {
        if self.params_ext.is_null() {
            return None;
        }

        unsafe {
            let ext = &*self.params_ext;
            let get_value = ext.get_value?;
            let mut value = 0.0;
            if get_value(self.plugin as *const c_void, param_id, &mut value) {
                Some(value)
            } else {
                None
            }
        }
    }

    /// Save plugin state
    pub fn save_state(&self) -> Option<Vec<u8>> {
        if self.state_ext.is_null() {
            return None;
        }

        unsafe {
            let ext = &*self.state_ext;
            let save = ext.save?;
            
            let mut buffer = StateBuffer::new();
            let stream = create_ostream(&mut buffer);
            
            if save(self.plugin as *const c_void, &stream) {
                Some(buffer.into_data())
            } else {
                None
            }
        }
    }

    /// Load plugin state
    pub fn load_state(&mut self, data: &[u8]) -> bool {
        if self.state_ext.is_null() {
            return false;
        }

        unsafe {
            let ext = &*self.state_ext;
            let load = match ext.load {
                Some(f) => f,
                None => return false,
            };
            
            let mut buffer = StateBuffer::from_data(data.to_vec());
            let stream = create_istream(&mut buffer);
            
            load(self.plugin as *const c_void, &stream)
        }
    }

    /// Get latency in samples
    pub fn get_latency(&self) -> u32 {
        if self.latency_ext.is_null() {
            return 0;
        }

        unsafe {
            let ext = &*self.latency_ext;
            ext.get.map(|f| f(self.plugin as *const c_void)).unwrap_or(0)
        }
    }

    /// Get tail in samples
    pub fn get_tail(&self) -> u32 {
        if self.tail_ext.is_null() {
            return 0;
        }

        unsafe {
            let ext = &*self.tail_ext;
            ext.get.map(|f| f(self.plugin as *const c_void)).unwrap_or(0)
        }
    }

    /// Get audio port count
    pub fn audio_port_count(&self, is_input: bool) -> u32 {
        if self.audio_ports_ext.is_null() {
            return if is_input { 1 } else { 1 };
        }

        unsafe {
            let ext = &*self.audio_ports_ext;
            ext.count.map(|f| f(self.plugin as *const c_void, is_input)).unwrap_or(1)
        }
    }

    /// Get note port count
    pub fn note_port_count(&self, is_input: bool) -> u32 {
        if self.note_ports_ext.is_null() {
            return 0;
        }

        unsafe {
            let ext = &*self.note_ports_ext;
            ext.count.map(|f| f(self.plugin as *const c_void, is_input)).unwrap_or(0)
        }
    }

    /// Activate the plugin for processing
    pub fn activate(&mut self, sample_rate: f64, max_frames: u32) -> Result<()> {
        if self.activated {
            return Ok(());
        }

        unsafe {
            let plugin_ref = &*self.plugin;
            if let Some(activate) = plugin_ref.activate {
                if !activate(self.plugin, sample_rate, 1, max_frames) {
                    return Err(Error::Plugin("Activation failed".into()));
                }
            }
        }

        self.sample_rate = sample_rate as f32;
        self.activated = true;
        Ok(())
    }

    /// Deactivate the plugin
    pub fn deactivate(&mut self) {
        if !self.activated {
            return;
        }

        if self.processing {
            self.stop_processing();
        }

        unsafe {
            let plugin_ref = &*self.plugin;
            if let Some(deactivate) = plugin_ref.deactivate {
                deactivate(self.plugin);
            }
        }

        self.activated = false;
    }

    /// Start processing
    pub fn start_processing(&mut self) -> Result<()> {
        if self.processing {
            return Ok(());
        }

        if !self.activated {
            self.activate(self.sample_rate as f64, 4096)?;
        }

        unsafe {
            let plugin_ref = &*self.plugin;
            if let Some(start) = plugin_ref.start_processing {
                if !start(self.plugin) {
                    return Err(Error::Plugin("Start processing failed".into()));
                }
            }
        }

        self.processing = true;
        Ok(())
    }

    /// Stop processing
    pub fn stop_processing(&mut self) {
        if !self.processing {
            return;
        }

        unsafe {
            let plugin_ref = &*self.plugin;
            if let Some(stop) = plugin_ref.stop_processing {
                stop(self.plugin);
            }
        }

        self.processing = false;
    }
}

unsafe extern "C" fn null_get_extension(_plugin: *const ClapPlugin, _id: *const c_char) -> *const c_void {
    ptr::null()
}

impl Drop for ClapInstance {
    fn drop(&mut self) {
        self.deactivate();
        
        unsafe {
            let plugin_ref = &*self.plugin;
            if let Some(destroy) = plugin_ref.destroy {
                destroy(self.plugin);
            }
        }
    }
}

impl Plugin for ClapInstance {
    fn info(&self) -> PluginInfo {
        self.info.clone()
    }

    fn init(&mut self, config: &PluginConfig) -> Result<()> {
        self.activate(config.sample_rate as f64, config.max_block_size as u32)?;
        self.start_processing()
    }

    fn process(&mut self, buffer: &mut AudioBuffer, _ctx: &ProcessContext) {
        if !self.processing {
            return;
        }

        let frames = buffer.frames();
        
        for buf in &mut self.input_buffer {
            if buf.len() < frames {
                buf.resize(frames, 0.0);
            }
        }
        for buf in &mut self.output_buffer {
            if buf.len() < frames {
                buf.resize(frames, 0.0);
            }
        }

        for (ch, buf) in self.input_buffer.iter_mut().enumerate() {
            if let Some(channel) = buffer.channel(ch) {
                buf[..frames].copy_from_slice(&channel[..frames]);
            }
        }

        for (i, buf) in self.input_buffer.iter_mut().enumerate() {
            self.input_ptrs[i] = buf.as_mut_ptr();
        }
        for (i, buf) in self.output_buffer.iter_mut().enumerate() {
            self.output_ptrs[i] = buf.as_mut_ptr();
        }

        let mut input_audio = ClapAudioBuffer {
            data32: self.input_ptrs.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        };

        let mut output_audio = ClapAudioBuffer {
            data32: self.output_ptrs.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        };

        let in_events = ClapInputEvents {
            ctx: ptr::null_mut(),
            size: Some(empty_event_size),
            get: Some(empty_event_get),
        };

        let out_events = ClapOutputEvents {
            ctx: ptr::null_mut(),
            try_push: Some(empty_event_push),
        };

        let process = ClapProcess {
            steady_time: -1,
            frames_count: frames as u32,
            transport: ptr::null(),
            audio_inputs: &input_audio,
            audio_outputs: &mut output_audio,
            audio_inputs_count: 1,
            audio_outputs_count: 1,
            in_events: &in_events,
            out_events: &out_events,
        };

        unsafe {
            let plugin_ref = &*self.plugin;
            if let Some(process_fn) = plugin_ref.process {
                process_fn(self.plugin, &process);
            }
        }

        for (ch, buf) in self.output_buffer.iter().enumerate() {
            if let Some(channel) = buffer.channel_mut(ch) {
                channel[..frames].copy_from_slice(&buf[..frames]);
            }
        }
    }

    fn set_parameter(&mut self, id: u32, value: f32) {
        // Parameters are set via events in CLAP, but we can flush if extension supports it
        if !self.params_ext.is_null() {
            unsafe {
                let ext = &*self.params_ext;
                if let Some(flush) = ext.flush {
                    // Would need to create param change event and flush
                    // For now, this is a stub
                    let _ = flush;
                    let _ = id;
                    let _ = value;
                }
            }
        }
    }

    fn get_parameter(&self, id: u32) -> f32 {
        self.get_param_value(id).unwrap_or(0.0) as f32
    }

    fn get_state(&self) -> Vec<u8> {
        self.save_state().unwrap_or_default()
    }

    fn set_state(&mut self, data: &[u8]) -> Result<()> {
        if self.load_state(data) {
            Ok(())
        } else {
            Err(Error::Plugin("Failed to load state".into()))
        }
    }

    fn reset(&mut self) {
        unsafe {
            let plugin_ref = &*self.plugin;
            if let Some(reset) = plugin_ref.reset {
                reset(self.plugin);
            }
        }
    }

    fn latency(&self) -> u32 {
        self.get_latency()
    }

    fn tail(&self) -> u32 {
        self.get_tail()
    }
}

unsafe extern "C" fn empty_event_size(_list: *const ClapInputEvents) -> u32 {
    0
}

unsafe extern "C" fn empty_event_get(_list: *const ClapInputEvents, _index: u32) -> *const ClapEventHeader {
    ptr::null()
}

unsafe extern "C" fn empty_event_push(_list: *const ClapOutputEvents, _event: *const ClapEventHeader) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clap_instance_info() {
        let _version = ClapVersion::default();
    }
}
