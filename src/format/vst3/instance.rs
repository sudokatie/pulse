//! VST3 plugin instance

use super::host::*;
use super::{get_vst3_binary_path, iid};
use crate::buffer::AudioBuffer;
use crate::plugin::{Plugin, PluginCategory, PluginConfig, PluginInfo};
use crate::process::ProcessContext as PulseProcessContext;
use crate::{Error, Result};
use libloading::{Library, Symbol};
use std::ffi::{c_char, c_void, CStr};
use std::path::Path;
use std::ptr;

type GetPluginFactoryFn = unsafe extern "system" fn() -> *mut IPluginFactory;
type InitDllFn = unsafe extern "system" fn() -> bool;
type ExitDllFn = unsafe extern "system" fn() -> bool;

/// VST3 plugin instance
pub struct Vst3Instance {
    _library: Library,
    component: *mut IComponent,
    processor: *mut IAudioProcessor,
    info: PluginInfo,
    sample_rate: f64,
    max_block_size: i32,
    activated: bool,
    processing: bool,
    // Audio buffers
    input_buffers: Vec<Vec<f32>>,
    output_buffers: Vec<Vec<f32>>,
    input_ptrs: Vec<*mut f32>,
    output_ptrs: Vec<*mut f32>,
}

// Safety: VST3Instance manages its own thread safety
unsafe impl Send for Vst3Instance {}
unsafe impl Sync for Vst3Instance {}

impl Vst3Instance {
    /// Load a VST3 plugin from a bundle path
    pub fn load(bundle_path: &Path, class_index: i32) -> Result<Self> {
        let binary_path = get_vst3_binary_path(bundle_path);
        
        // Check if binary exists
        if !binary_path.exists() {
            return Err(Error::Plugin(format!(
                "VST3 binary not found at {:?}", binary_path
            )));
        }

        // Load the library
        let library = unsafe {
            Library::new(&binary_path)
                .map_err(|e| Error::Plugin(format!("Failed to load VST3: {}", e)))?
        };

        // Initialize the module
        let init_result = unsafe {
            if let Ok(init_dll) = library.get::<InitDllFn>(b"InitDll") {
                init_dll()
            } else {
                true // InitDll is optional
            }
        };

        if !init_result {
            return Err(Error::Plugin("VST3 InitDll failed".into()));
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

        // Get factory vtable
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
            let vtable = (*(*component).unknown.vtable);
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

        // Extract name from class info
        let name = unsafe {
            let name_slice = &class_info.name[..];
            let end = name_slice.iter().position(|&c| c == 0).unwrap_or(name_slice.len());
            String::from_utf8_lossy(&name_slice[..end]).to_string()
        };

        // Extract category
        let category = unsafe {
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
            component,
            processor,
            info,
            sample_rate: 44100.0,
            max_block_size: 4096,
            activated: false,
            processing: false,
            input_buffers: vec![vec![0.0; 4096]; 2],
            output_buffers: vec![vec![0.0; 4096]; 2],
            input_ptrs: vec![ptr::null_mut(); 2],
            output_ptrs: vec![ptr::null_mut(); 2],
        })
    }

    /// Setup processing
    fn setup_processing(&mut self) -> Result<()> {
        // This would call IAudioProcessor::setupProcessing
        // For now, we just mark as set up
        Ok(())
    }

    /// Set active state
    fn set_active(&mut self, active: bool) -> Result<()> {
        // This would call IComponent::setActive
        self.activated = active;
        Ok(())
    }

    /// Start processing
    fn start_processing(&mut self) -> Result<()> {
        // This would call IAudioProcessor::setProcessing(true)
        self.processing = true;
        Ok(())
    }

    /// Stop processing
    fn stop_processing(&mut self) {
        // This would call IAudioProcessor::setProcessing(false)
        self.processing = false;
    }
}

impl Drop for Vst3Instance {
    fn drop(&mut self) {
        // Release interfaces
        unsafe {
            if !self.processor.is_null() {
                let vtable = (*(*self.processor).unknown.vtable);
                (vtable.release)(self.processor as *mut FUnknown);
            }
            if !self.component.is_null() {
                let vtable = (*(*self.component).unknown.vtable);
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
        self.start_processing()
    }

    fn process(&mut self, buffer: &mut AudioBuffer, ctx: &PulseProcessContext) {
        if !self.processing {
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

        // Create process data
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
            process_mode: 0, // Realtime
            symbolic_sample_size: 0, // 32-bit float
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

        // Process would be called here if we had full vtable binding
        // For now, just copy input to output as passthrough
        for (ch, buf) in self.output_buffers.iter().enumerate() {
            if let Some(channel) = buffer.channel_mut(ch) {
                channel[..frames].copy_from_slice(&buf[..frames]);
            }
        }
    }

    fn set_parameter(&mut self, _id: u32, _value: f32) {
        // Would use IEditController
    }

    fn get_parameter(&self, _id: u32) -> f32 {
        0.0
    }

    fn reset(&mut self) {
        // Reset internal state
        for buf in &mut self.input_buffers {
            buf.fill(0.0);
        }
        for buf in &mut self.output_buffers {
            buf.fill(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vst3_binary_path() {
        let bundle = Path::new("/Library/Audio/Plug-Ins/VST3/MyPlugin.vst3");
        let _binary = get_vst3_binary_path(bundle);
        // Just verify it compiles and returns something
    }
}
