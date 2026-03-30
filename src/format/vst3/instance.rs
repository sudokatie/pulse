//! VST3 plugin instance with IEditController and state support

use super::host::*;
use super::{get_vst3_binary_path, iid};
use crate::buffer::AudioBuffer;
use crate::plugin::{Plugin, PluginCategory, PluginConfig, PluginInfo};
use crate::process::ProcessContext as PulseProcessContext;
use crate::{Error, Result};
use libloading::{Library, Symbol};
use std::ffi::c_void;
use std::path::Path;
use std::ptr;

type GetPluginFactoryFn = unsafe extern "system" fn() -> *mut IPluginFactory;
type InitDllFn = unsafe extern "system" fn() -> bool;

/// VST3 plugin instance
pub struct Vst3Instance {
    _library: Library,
    factory: *mut IPluginFactory,
    component: *mut IComponent,
    processor: *mut IAudioProcessor,
    controller: *mut IEditController,
    info: PluginInfo,
    sample_rate: f64,
    max_block_size: i32,
    activated: bool,
    processing: bool,
    param_count: i32,
    // Audio buffers
    input_buffers: Vec<Vec<f32>>,
    output_buffers: Vec<Vec<f32>>,
    input_ptrs: Vec<*mut f32>,
    output_ptrs: Vec<*mut f32>,
    // State handling
    component_state: Vec<u8>,
    controller_state: Vec<u8>,
}

// Safety: VST3Instance manages its own thread safety
unsafe impl Send for Vst3Instance {}
unsafe impl Sync for Vst3Instance {}

impl Vst3Instance {
    /// Load a VST3 plugin from a bundle path
    pub fn load(bundle_path: &Path, class_index: i32) -> Result<Self> {
        let binary_path = get_vst3_binary_path(bundle_path);
        
        if !binary_path.exists() {
            return Err(Error::Plugin(format!(
                "VST3 binary not found at {:?}", binary_path
            )));
        }

        let library = unsafe {
            Library::new(&binary_path)
                .map_err(|e| Error::Plugin(format!("Failed to load VST3: {}", e)))?
        };

        // Initialize the module
        unsafe {
            if let Ok(init_dll) = library.get::<InitDllFn>(b"InitDll") {
                if !init_dll() {
                    return Err(Error::Plugin("VST3 InitDll failed".into()));
                }
            }
        }

        // Get the factory
        let get_factory: Symbol<GetPluginFactoryFn> = unsafe {
            library.get(b"GetPluginFactory")
                .map_err(|e| Error::Plugin(format!("No GetPluginFactory: {}", e)))?
        };

        let factory = unsafe { get_factory() };
        if factory.is_null() {
            return Err(Error::Plugin("Null factory".into()));
        }

        let factory_vtable = unsafe {
            let vtable_ptr = (*factory).unknown.vtable as *const IPluginFactoryVtable;
            &*vtable_ptr
        };

        // Get class info
        let mut class_info = PClassInfo {
            cid: [0; 16],
            cardinality: 0,
            category: [0; 32],
            name: [0; 64],
        };

        let result = unsafe {
            (factory_vtable.get_class_info)(factory, class_index, &mut class_info)
        };

        if result != 0 {
            return Err(Error::Plugin("Failed to get class info".into()));
        }

        // Create the component instance
        let mut component: *mut c_void = ptr::null_mut();
        let result = unsafe {
            (factory_vtable.create_instance)(
                factory,
                &class_info.cid,
                &iid::ICOMPONENT,
                &mut component
            )
        };

        if result != 0 || component.is_null() {
            return Err(Error::Plugin("Failed to create component".into()));
        }

        let component = component as *mut IComponent;

        // Query for IAudioProcessor
        let mut processor: *mut c_void = ptr::null_mut();
        let result = unsafe {
            let vtable = *(*component).unknown.vtable;
            (vtable.query_interface)(
                component as *mut FUnknown,
                &iid::IAUDIO_PROCESSOR,
                &mut processor
            )
        };

        if result != 0 || processor.is_null() {
            return Err(Error::Plugin("Plugin doesn't support IAudioProcessor".into()));
        }

        let processor = processor as *mut IAudioProcessor;

        // Query for IEditController (optional - some plugins expose it separately)
        let mut controller: *mut c_void = ptr::null_mut();
        let controller = unsafe {
            let vtable = *(*component).unknown.vtable;
            let result = (vtable.query_interface)(
                component as *mut FUnknown,
                &iid::IEDIT_CONTROLLER,
                &mut controller
            );
            if result == 0 && !controller.is_null() {
                controller as *mut IEditController
            } else {
                // Try to create controller from controller class ID
                let component_vtable = (*component).unknown.vtable as *const IComponentVtable;
                let mut controller_cid = [0u8; 16];
                if ((*component_vtable).get_controller_class_id)(component, &mut controller_cid) == 0 {
                    let mut ctrl: *mut c_void = ptr::null_mut();
                    if (factory_vtable.create_instance)(
                        factory,
                        &controller_cid,
                        &iid::IEDIT_CONTROLLER,
                        &mut ctrl
                    ) == 0 && !ctrl.is_null() {
                        ctrl as *mut IEditController
                    } else {
                        ptr::null_mut()
                    }
                } else {
                    ptr::null_mut()
                }
            }
        };

        // Get parameter count if controller exists
        let param_count = if !controller.is_null() {
            unsafe {
                let vtable = (*controller).unknown.vtable as *const IEditControllerVtable;
                ((*vtable).get_parameter_count)(controller)
            }
        } else {
            0
        };

        // Extract name from class info
        let name = {
            let name_slice = &class_info.name[..];
            let end = name_slice.iter().position(|&c| c == 0).unwrap_or(name_slice.len());
            String::from_utf8_lossy(&name_slice[..end]).to_string()
        };

        // Extract category
        let category = {
            let cat_slice = &class_info.category[..];
            let end = cat_slice.iter().position(|&c| c == 0).unwrap_or(cat_slice.len());
            String::from_utf8_lossy(&cat_slice[..end]).to_string()
        };

        let plugin_category = if category.contains("Fx") || category.contains("Effect") {
            PluginCategory::Effect
        } else if category.contains("Instrument") || category.contains("VSTi") {
            PluginCategory::Instrument
        } else {
            PluginCategory::Effect
        };

        let info = PluginInfo {
            id: format!("{:02X?}", class_info.cid).replace(", ", ""),
            name,
            vendor: "Unknown".to_string(),
            version: "1.0.0".to_string(),
            category: plugin_category,
            inputs: 2,
            outputs: 2,
        };

        Ok(Self {
            _library: library,
            factory,
            component,
            processor,
            controller,
            info,
            sample_rate: 44100.0,
            max_block_size: 4096,
            activated: false,
            processing: false,
            param_count,
            input_buffers: vec![vec![0.0; 4096]; 2],
            output_buffers: vec![vec![0.0; 4096]; 2],
            input_ptrs: vec![ptr::null_mut(); 2],
            output_ptrs: vec![ptr::null_mut(); 2],
            component_state: Vec::new(),
            controller_state: Vec::new(),
        })
    }

    /// Get parameter count
    pub fn parameter_count(&self) -> i32 {
        self.param_count
    }

    /// Get parameter info
    pub fn parameter_info(&self, index: i32) -> Option<(u32, String)> {
        if self.controller.is_null() || index < 0 || index >= self.param_count {
            return None;
        }

        unsafe {
            let vtable = (*self.controller).unknown.vtable as *const IEditControllerVtable;
            let mut info = ParameterInfo {
                id: 0,
                title: [0; 128],
                short_title: [0; 128],
                units: [0; 128],
                step_count: 0,
                default_normalized_value: 0.0,
                unit_id: 0,
                flags: 0,
            };
            
            if ((*vtable).get_parameter_info)(self.controller, index, &mut info) == 0 {
                let title = String::from_utf16_lossy(&info.title)
                    .trim_end_matches('\0')
                    .to_string();
                Some((info.id, title))
            } else {
                None
            }
        }
    }

    /// Set parameter by ID (normalized 0-1)
    pub fn set_param_normalized(&mut self, id: u32, value: f64) -> bool {
        if self.controller.is_null() {
            return false;
        }

        unsafe {
            let vtable = (*self.controller).unknown.vtable as *const IEditControllerVtable;
            ((*vtable).set_param_normalized)(self.controller, id, value) == 0
        }
    }

    /// Get parameter by ID (normalized 0-1)
    pub fn get_param_normalized(&self, id: u32) -> f64 {
        if self.controller.is_null() {
            return 0.0;
        }

        unsafe {
            let vtable = (*self.controller).unknown.vtable as *const IEditControllerVtable;
            ((*vtable).get_param_normalized)(self.controller, id)
        }
    }

    /// Convert normalized to plain value
    pub fn param_to_plain(&self, id: u32, normalized: f64) -> f64 {
        if self.controller.is_null() {
            return normalized;
        }

        unsafe {
            let vtable = (*self.controller).unknown.vtable as *const IEditControllerVtable;
            ((*vtable).normalized_param_to_plain)(self.controller, id, normalized)
        }
    }

    /// Convert plain to normalized value
    pub fn param_to_normalized(&self, id: u32, plain: f64) -> f64 {
        if self.controller.is_null() {
            return plain;
        }

        unsafe {
            let vtable = (*self.controller).unknown.vtable as *const IEditControllerVtable;
            ((*vtable).plain_param_to_normalized)(self.controller, id, plain)
        }
    }

    /// Setup processing
    fn setup_processing(&mut self) -> Result<()> {
        if self.processor.is_null() {
            return Ok(());
        }

        let setup = ProcessSetup {
            process_mode: 0, // Realtime
            symbolic_sample_size: 0, // 32-bit float
            max_samples_per_block: self.max_block_size,
            sample_rate: self.sample_rate,
        };

        unsafe {
            let vtable = (*self.processor).unknown.vtable as *const IAudioProcessorVtable;
            let result = ((*vtable).setup_processing)(self.processor, &setup);
            if result != 0 {
                return Err(Error::Plugin(format!("setup_processing failed: {}", result)));
            }
        }

        Ok(())
    }

    /// Set active state
    fn set_active(&mut self, active: bool) -> Result<()> {
        if self.component.is_null() {
            return Ok(());
        }

        unsafe {
            let vtable = (*self.component).unknown.vtable as *const IComponentVtable;
            let result = ((*vtable).set_active)(self.component, if active { 1 } else { 0 });
            if result != 0 {
                return Err(Error::Plugin(format!("set_active failed: {}", result)));
            }
        }

        self.activated = active;
        Ok(())
    }

    /// Start/stop processing
    fn set_processing(&mut self, state: bool) -> Result<()> {
        if self.processor.is_null() {
            return Ok(());
        }

        unsafe {
            let vtable = (*self.processor).unknown.vtable as *const IAudioProcessorVtable;
            let result = ((*vtable).set_processing)(self.processor, if state { 1 } else { 0 });
            if result != 0 {
                return Err(Error::Plugin(format!("set_processing failed: {}", result)));
            }
        }

        self.processing = state;
        Ok(())
    }

    /// Get latency in samples
    pub fn get_latency(&self) -> u32 {
        if self.processor.is_null() {
            return 0;
        }

        unsafe {
            let vtable = (*self.processor).unknown.vtable as *const IAudioProcessorVtable;
            ((*vtable).get_latency_samples)(self.processor)
        }
    }

    /// Get tail samples
    pub fn get_tail(&self) -> u32 {
        if self.processor.is_null() {
            return 0;
        }

        unsafe {
            let vtable = (*self.processor).unknown.vtable as *const IAudioProcessorVtable;
            ((*vtable).get_tail_samples)(self.processor)
        }
    }
}

impl Drop for Vst3Instance {
    fn drop(&mut self) {
        // Deactivate and stop processing first
        let _ = self.set_processing(false);
        let _ = self.set_active(false);

        // Release interfaces
        unsafe {
            if !self.controller.is_null() {
                let vtable = *(*self.controller).unknown.vtable;
                (vtable.release)(self.controller as *mut FUnknown);
            }
            if !self.processor.is_null() {
                let vtable = *(*self.processor).unknown.vtable;
                (vtable.release)(self.processor as *mut FUnknown);
            }
            if !self.component.is_null() {
                let vtable = *(*self.component).unknown.vtable;
                (vtable.release)(self.component as *mut FUnknown);
            }
        }
    }
}

impl Plugin for Vst3Instance {
    fn info(&self) -> PluginInfo {
        self.info.clone()
    }

    fn init(&mut self, config: &PluginConfig) -> Result<()> {
        self.sample_rate = config.sample_rate as f64;
        self.max_block_size = config.max_block_size as i32;
        
        // Resize buffers
        for buf in &mut self.input_buffers {
            buf.resize(self.max_block_size as usize, 0.0);
        }
        for buf in &mut self.output_buffers {
            buf.resize(self.max_block_size as usize, 0.0);
        }
        
        self.setup_processing()?;
        self.set_active(true)?;
        self.set_processing(true)
    }

    fn process(&mut self, buffer: &mut AudioBuffer, ctx: &PulseProcessContext) {
        if !self.processing || self.processor.is_null() {
            return;
        }

        let frames = buffer.frames().min(self.max_block_size as usize);

        // Copy input
        for (ch, buf) in self.input_buffers.iter_mut().enumerate() {
            if let Some(channel) = buffer.channel(ch) {
                buf[..frames].copy_from_slice(&channel[..frames]);
            }
        }

        // Set up pointers
        for (i, buf) in self.input_buffers.iter_mut().enumerate() {
            self.input_ptrs[i] = buf.as_mut_ptr();
        }
        for (i, buf) in self.output_buffers.iter_mut().enumerate() {
            self.output_ptrs[i] = buf.as_mut_ptr();
        }

        let mut input_bus = AudioBusBuffers {
            num_channels: 2,
            silence_flags: 0,
            channel_buffers32: self.input_ptrs.as_mut_ptr(),
            channel_buffers64: ptr::null_mut(),
        };

        let mut output_bus = AudioBusBuffers {
            num_channels: 2,
            silence_flags: 0,
            channel_buffers32: self.output_ptrs.as_mut_ptr(),
            channel_buffers64: ptr::null_mut(),
        };

        let mut process_ctx = ProcessContext {
            state: 0,
            sample_rate: self.sample_rate,
            project_time_samples: 0,
            system_time: 0,
            continuous_time_samples: 0,
            project_time_music: 0.0,
            bar_position_music: 0.0,
            cycle_start_music: 0.0,
            cycle_end_music: 0.0,
            tempo: ctx.tempo,
            time_sig_numerator: ctx.time_sig.0 as i32,
            time_sig_denominator: ctx.time_sig.1 as i32,
            chord: 0,
            smpte_offset_subframes: 0,
            frame_rate: 0,
            samples_to_next_clock: 0,
        };

        let mut process_data = ProcessData {
            process_mode: 0,
            symbolic_sample_size: 0,
            num_samples: frames as i32,
            num_inputs: 1,
            num_outputs: 1,
            inputs: &mut input_bus,
            outputs: &mut output_bus,
            input_param_changes: ptr::null_mut(),
            output_param_changes: ptr::null_mut(),
            input_events: ptr::null_mut(),
            output_events: ptr::null_mut(),
            context: &mut process_ctx,
        };

        // Call process
        unsafe {
            let vtable = (*self.processor).unknown.vtable as *const IAudioProcessorVtable;
            ((*vtable).process)(self.processor, &mut process_data);
        }

        // Copy output
        for (ch, buf) in self.output_buffers.iter().enumerate() {
            if let Some(channel) = buffer.channel_mut(ch) {
                channel[..frames].copy_from_slice(&buf[..frames]);
            }
        }
    }

    fn set_parameter(&mut self, id: u32, value: f32) {
        self.set_param_normalized(id, value as f64);
    }

    fn get_parameter(&self, id: u32) -> f32 {
        self.get_param_normalized(id) as f32
    }

    fn get_state(&self) -> Vec<u8> {
        // Return combined component + controller state
        let mut state = Vec::new();
        state.extend_from_slice(&self.component_state);
        state.extend_from_slice(&self.controller_state);
        state
    }

    fn set_state(&mut self, data: &[u8]) -> Result<()> {
        // Store state for restoration
        self.component_state = data.to_vec();
        Ok(())
    }

    fn reset(&mut self) {
        for buf in &mut self.input_buffers {
            buf.fill(0.0);
        }
        for buf in &mut self.output_buffers {
            buf.fill(0.0);
        }
    }

    fn latency(&self) -> u32 {
        self.get_latency()
    }

    fn tail(&self) -> u32 {
        self.get_tail()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vst3_binary_path() {
        let bundle = Path::new("/Library/Audio/Plug-Ins/VST3/MyPlugin.vst3");
        let _binary = get_vst3_binary_path(bundle);
    }

    #[test]
    fn test_memory_stream_for_state() {
        let mut stream = MemoryStream::new();
        stream.write(b"test state data");
        assert_eq!(stream.data(), b"test state data");
    }
}
