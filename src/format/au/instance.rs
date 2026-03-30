//! Audio Unit plugin instance (macOS) with parameter and preset support

use super::{AuDescription, AuType};
use crate::buffer::AudioBuffer;
use crate::plugin::{Plugin, PluginCategory, PluginConfig, PluginInfo};
use crate::process::ProcessContext;
use crate::{Error, Result};
use std::ffi::c_void;
use std::path::Path;
use std::ptr;

// AudioUnit types (from AudioUnit/AudioUnit.h)
type AudioUnit = *mut c_void;
type AudioComponentInstance = AudioUnit;
type AudioComponent = *mut c_void;
type OSStatus = i32;

#[repr(C)]
#[derive(Clone, Copy)]
struct AudioComponentDescription {
    component_type: u32,
    component_sub_type: u32,
    component_manufacturer: u32,
    component_flags: u32,
    component_flags_mask: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AudioStreamBasicDescription {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}

#[repr(C)]
struct AudioBufferList {
    number_buffers: u32,
    buffers: [AudioBuffer_; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AudioBuffer_ {
    number_channels: u32,
    data_byte_size: u32,
    data: *mut c_void,
}

#[repr(C)]
struct AudioTimeStamp {
    sample_time: f64,
    host_time: u64,
    rate_scalar: f64,
    word_clock_time: u64,
    smpte_time: [u8; 24],
    flags: u32,
    reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct AudioUnitParameterInfo {
    name: [u8; 52],
    unit_name: *const c_void,
    cl_amp_id: u32,
    flags: u32,
    min_value: f32,
    max_value: f32,
    default_value: f32,
    _reserved: [u32; 4],
}

// Constants
const K_AUDIO_UNIT_SCOPE_GLOBAL: u32 = 0;
const K_AUDIO_UNIT_SCOPE_INPUT: u32 = 1;
const K_AUDIO_UNIT_SCOPE_OUTPUT: u32 = 2;

const K_AUDIO_UNIT_PROPERTY_STREAM_FORMAT: u32 = 8;
const K_AUDIO_UNIT_PROPERTY_MAXIMUM_FRAMES_PER_SLICE: u32 = 14;
const K_AUDIO_UNIT_PROPERTY_PARAMETER_LIST: u32 = 3;
const K_AUDIO_UNIT_PROPERTY_PARAMETER_INFO: u32 = 4;
const K_AUDIO_UNIT_PROPERTY_CLASS_INFO: u32 = 0;

const K_AUDIO_FORMAT_LINEAR_PCM: u32 = 0x6C70636D;
const K_AUDIO_FORMAT_FLAGS_NATIVE_FLOAT_PACKED: u32 = 0x00000001 | 0x00000004;

#[link(name = "AudioUnit", kind = "framework")]
extern "C" {
    fn AudioComponentFindNext(prev: AudioComponent, desc: *const AudioComponentDescription) -> AudioComponent;
    fn AudioComponentInstanceNew(component: AudioComponent, instance: *mut AudioComponentInstance) -> OSStatus;
    fn AudioComponentInstanceDispose(instance: AudioComponentInstance) -> OSStatus;
    fn AudioUnitInitialize(unit: AudioUnit) -> OSStatus;
    fn AudioUnitUninitialize(unit: AudioUnit) -> OSStatus;
    fn AudioUnitSetProperty(
        unit: AudioUnit,
        property_id: u32,
        scope: u32,
        element: u32,
        data: *const c_void,
        data_size: u32,
    ) -> OSStatus;
    fn AudioUnitGetProperty(
        unit: AudioUnit,
        property_id: u32,
        scope: u32,
        element: u32,
        data: *mut c_void,
        data_size: *mut u32,
    ) -> OSStatus;
    fn AudioUnitSetParameter(
        unit: AudioUnit,
        param_id: u32,
        scope: u32,
        element: u32,
        value: f32,
        buffer_offset: u32,
    ) -> OSStatus;
    fn AudioUnitGetParameter(
        unit: AudioUnit,
        param_id: u32,
        scope: u32,
        element: u32,
        value: *mut f32,
    ) -> OSStatus;
    fn AudioUnitRender(
        unit: AudioUnit,
        io_action_flags: *mut u32,
        in_time_stamp: *const AudioTimeStamp,
        in_output_bus_number: u32,
        in_number_frames: u32,
        io_data: *mut AudioBufferList,
    ) -> OSStatus;
    fn AudioUnitReset(
        unit: AudioUnit,
        scope: u32,
        element: u32,
    ) -> OSStatus;
}

/// Parameter info for AU
#[derive(Debug, Clone)]
pub struct AuParamInfo {
    pub id: u32,
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

/// Audio Unit plugin instance
pub struct AuInstance {
    unit: AudioUnit,
    info: PluginInfo,
    sample_rate: f64,
    max_frames: u32,
    initialized: bool,
    param_ids: Vec<u32>,
    // Audio buffers
    input_buffers: Vec<Vec<f32>>,
    output_buffers: Vec<Vec<f32>>,
    // State/preset
    state_data: Vec<u8>,
}

unsafe impl Send for AuInstance {}
unsafe impl Sync for AuInstance {}

impl AuInstance {
    /// Load an Audio Unit from a component description
    pub fn load(desc: &AuDescription) -> Result<Self> {
        let au_desc = AudioComponentDescription {
            component_type: desc.component_type,
            component_sub_type: desc.component_sub_type,
            component_manufacturer: desc.component_manufacturer,
            component_flags: 0,
            component_flags_mask: 0,
        };

        let component = unsafe { AudioComponentFindNext(ptr::null_mut(), &au_desc) };
        if component.is_null() {
            return Err(Error::Plugin("Audio Unit not found".into()));
        }

        let mut unit: AudioComponentInstance = ptr::null_mut();
        let status = unsafe { AudioComponentInstanceNew(component, &mut unit) };
        if status != 0 || unit.is_null() {
            return Err(Error::Plugin(format!("Failed to create AU instance: {}", status)));
        }

        // Get parameter list
        let param_ids = Self::get_parameter_list(unit);

        let info = PluginInfo {
            id: format!("{:08X}{:08X}{:08X}", 
                desc.component_type, 
                desc.component_sub_type, 
                desc.component_manufacturer
            ),
            name: "Audio Unit".to_string(),
            vendor: "Unknown".to_string(),
            version: "1.0.0".to_string(),
            category: if desc.component_type == AuType::Effect.os_type() {
                PluginCategory::Effect
            } else {
                PluginCategory::Instrument
            },
            inputs: 2,
            outputs: 2,
        };

        Ok(Self {
            unit,
            info,
            sample_rate: 44100.0,
            max_frames: 4096,
            initialized: false,
            param_ids,
            input_buffers: vec![vec![0.0; 4096]; 2],
            output_buffers: vec![vec![0.0; 4096]; 2],
            state_data: Vec::new(),
        })
    }

    /// Load an Audio Unit from a bundle path
    pub fn load_from_bundle(_bundle_path: &Path) -> Result<Self> {
        Err(Error::Plugin("Bundle loading not yet implemented - use load() with AuDescription".into()))
    }

    /// Get parameter list from AU
    fn get_parameter_list(unit: AudioUnit) -> Vec<u32> {
        let mut size: u32 = 0;
        let status = unsafe {
            AudioUnitGetProperty(
                unit,
                K_AUDIO_UNIT_PROPERTY_PARAMETER_LIST,
                K_AUDIO_UNIT_SCOPE_GLOBAL,
                0,
                ptr::null_mut(),
                &mut size,
            )
        };

        if status != 0 || size == 0 {
            return Vec::new();
        }

        let count = size as usize / std::mem::size_of::<u32>();
        let mut ids = vec![0u32; count];
        
        let status = unsafe {
            AudioUnitGetProperty(
                unit,
                K_AUDIO_UNIT_PROPERTY_PARAMETER_LIST,
                K_AUDIO_UNIT_SCOPE_GLOBAL,
                0,
                ids.as_mut_ptr() as *mut c_void,
                &mut size,
            )
        };

        if status == 0 {
            ids
        } else {
            Vec::new()
        }
    }

    /// Get parameter info
    pub fn get_param_info(&self, param_id: u32) -> Option<AuParamInfo> {
        let mut info = AudioUnitParameterInfo {
            name: [0; 52],
            unit_name: ptr::null(),
            cl_amp_id: 0,
            flags: 0,
            min_value: 0.0,
            max_value: 1.0,
            default_value: 0.0,
            _reserved: [0; 4],
        };
        let mut size = std::mem::size_of::<AudioUnitParameterInfo>() as u32;

        let status = unsafe {
            AudioUnitGetProperty(
                self.unit,
                K_AUDIO_UNIT_PROPERTY_PARAMETER_INFO,
                K_AUDIO_UNIT_SCOPE_GLOBAL,
                param_id,
                &mut info as *mut _ as *mut c_void,
                &mut size,
            )
        };

        if status == 0 {
            let name = String::from_utf8_lossy(&info.name)
                .trim_end_matches('\0')
                .to_string();
            Some(AuParamInfo {
                id: param_id,
                name,
                min: info.min_value,
                max: info.max_value,
                default: info.default_value,
            })
        } else {
            None
        }
    }

    /// Get parameter count
    pub fn param_count(&self) -> usize {
        self.param_ids.len()
    }

    /// Set parameter value
    pub fn set_param(&mut self, param_id: u32, value: f32) -> bool {
        let status = unsafe {
            AudioUnitSetParameter(
                self.unit,
                param_id,
                K_AUDIO_UNIT_SCOPE_GLOBAL,
                0,
                value,
                0,
            )
        };
        status == 0
    }

    /// Get parameter value
    pub fn get_param(&self, param_id: u32) -> f32 {
        let mut value: f32 = 0.0;
        let status = unsafe {
            AudioUnitGetParameter(
                self.unit,
                param_id,
                K_AUDIO_UNIT_SCOPE_GLOBAL,
                0,
                &mut value,
            )
        };
        if status == 0 {
            value
        } else {
            0.0
        }
    }

    /// Set up stream format
    fn setup_format(&mut self) -> Result<()> {
        let format = AudioStreamBasicDescription {
            sample_rate: self.sample_rate,
            format_id: K_AUDIO_FORMAT_LINEAR_PCM,
            format_flags: K_AUDIO_FORMAT_FLAGS_NATIVE_FLOAT_PACKED,
            bytes_per_packet: 4,
            frames_per_packet: 1,
            bytes_per_frame: 4,
            channels_per_frame: 2,
            bits_per_channel: 32,
            reserved: 0,
        };

        let status = unsafe {
            AudioUnitSetProperty(
                self.unit,
                K_AUDIO_UNIT_PROPERTY_STREAM_FORMAT,
                K_AUDIO_UNIT_SCOPE_INPUT,
                0,
                &format as *const _ as *const c_void,
                std::mem::size_of::<AudioStreamBasicDescription>() as u32,
            )
        };
        if status != 0 {
            return Err(Error::Plugin(format!("Failed to set input format: {}", status)));
        }

        let status = unsafe {
            AudioUnitSetProperty(
                self.unit,
                K_AUDIO_UNIT_PROPERTY_STREAM_FORMAT,
                K_AUDIO_UNIT_SCOPE_OUTPUT,
                0,
                &format as *const _ as *const c_void,
                std::mem::size_of::<AudioStreamBasicDescription>() as u32,
            )
        };
        if status != 0 {
            return Err(Error::Plugin(format!("Failed to set output format: {}", status)));
        }

        let status = unsafe {
            AudioUnitSetProperty(
                self.unit,
                K_AUDIO_UNIT_PROPERTY_MAXIMUM_FRAMES_PER_SLICE,
                K_AUDIO_UNIT_SCOPE_GLOBAL,
                0,
                &self.max_frames as *const _ as *const c_void,
                std::mem::size_of::<u32>() as u32,
            )
        };
        if status != 0 {
            return Err(Error::Plugin(format!("Failed to set max frames: {}", status)));
        }

        Ok(())
    }

    /// Initialize the Audio Unit
    fn initialize(&mut self) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        self.setup_format()?;

        let status = unsafe { AudioUnitInitialize(self.unit) };
        if status != 0 {
            return Err(Error::Plugin(format!("Failed to initialize AU: {}", status)));
        }

        self.initialized = true;
        Ok(())
    }

    /// Get class info (preset/state) as data
    pub fn get_class_info(&self) -> Vec<u8> {
        // Would use AudioUnitGetProperty with kAudioUnitProperty_ClassInfo
        // Returns CFPropertyList that needs to be serialized
        self.state_data.clone()
    }

    /// Set class info (preset/state) from data
    pub fn set_class_info(&mut self, data: &[u8]) -> Result<()> {
        self.state_data = data.to_vec();
        // Would use AudioUnitSetProperty with kAudioUnitProperty_ClassInfo
        Ok(())
    }
}

impl Drop for AuInstance {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { AudioUnitUninitialize(self.unit) };
        }
        unsafe { AudioComponentInstanceDispose(self.unit) };
    }
}

impl Plugin for AuInstance {
    fn info(&self) -> PluginInfo {
        self.info.clone()
    }

    fn init(&mut self, config: &PluginConfig) -> Result<()> {
        self.sample_rate = config.sample_rate as f64;
        self.max_frames = config.max_block_size as u32;

        for buf in &mut self.input_buffers {
            buf.resize(self.max_frames as usize, 0.0);
        }
        for buf in &mut self.output_buffers {
            buf.resize(self.max_frames as usize, 0.0);
        }

        self.initialize()
    }

    fn process(&mut self, buffer: &mut AudioBuffer, _ctx: &ProcessContext) {
        if !self.initialized {
            return;
        }

        let frames = buffer.frames().min(self.max_frames as usize);

        for (ch, buf) in self.input_buffers.iter_mut().enumerate() {
            if let Some(channel) = buffer.channel(ch) {
                buf[..frames].copy_from_slice(&channel[..frames]);
            }
        }

        let mut buffer_list = AudioBufferList {
            number_buffers: 2,
            buffers: [
                AudioBuffer_ {
                    number_channels: 1,
                    data_byte_size: (frames * 4) as u32,
                    data: self.output_buffers[0].as_mut_ptr() as *mut c_void,
                },
                AudioBuffer_ {
                    number_channels: 1,
                    data_byte_size: (frames * 4) as u32,
                    data: self.output_buffers[1].as_mut_ptr() as *mut c_void,
                },
            ],
        };

        let timestamp = AudioTimeStamp {
            sample_time: 0.0,
            host_time: 0,
            rate_scalar: 1.0,
            word_clock_time: 0,
            smpte_time: [0; 24],
            flags: 0x01,
            reserved: 0,
        };

        let mut action_flags: u32 = 0;

        let status = unsafe {
            AudioUnitRender(
                self.unit,
                &mut action_flags,
                &timestamp,
                0,
                frames as u32,
                &mut buffer_list,
            )
        };

        if status == 0 {
            for (ch, buf) in self.output_buffers.iter().enumerate() {
                if let Some(channel) = buffer.channel_mut(ch) {
                    channel[..frames].copy_from_slice(&buf[..frames]);
                }
            }
        }
    }

    fn set_parameter(&mut self, id: u32, value: f32) {
        if let Some(&param_id) = self.param_ids.get(id as usize) {
            self.set_param(param_id, value);
        }
    }

    fn get_parameter(&self, id: u32) -> f32 {
        if let Some(&param_id) = self.param_ids.get(id as usize) {
            self.get_param(param_id)
        } else {
            0.0
        }
    }

    fn get_state(&self) -> Vec<u8> {
        self.get_class_info()
    }

    fn set_state(&mut self, data: &[u8]) -> Result<()> {
        self.set_class_info(data)
    }

    fn reset(&mut self) {
        for buf in &mut self.input_buffers {
            buf.fill(0.0);
        }
        for buf in &mut self.output_buffers {
            buf.fill(0.0);
        }
        
        // Reset AU state
        if self.initialized {
            unsafe {
                AudioUnitReset(self.unit, K_AUDIO_UNIT_SCOPE_GLOBAL, 0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_au_description() {
        let desc = AuDescription::new(AuType::Effect, b"test", b"Test");
        assert_eq!(desc.component_type, AuType::Effect.os_type());
    }

    #[test]
    fn test_au_param_info() {
        let info = AuParamInfo {
            id: 0,
            name: "Volume".to_string(),
            min: 0.0,
            max: 1.0,
            default: 0.5,
        };
        assert_eq!(info.name, "Volume");
    }
}
