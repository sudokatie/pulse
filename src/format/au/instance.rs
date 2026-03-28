//! Audio Unit plugin instance (macOS)

use super::{AuDescription, AuType, get_au_binary_path};
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
    smpte_time: [u8; 24], // SMPTETime struct simplified
    flags: u32,
    reserved: u32,
}

// Constants
const K_AUDIO_UNIT_SCOPE_GLOBAL: u32 = 0;
const K_AUDIO_UNIT_SCOPE_INPUT: u32 = 1;
const K_AUDIO_UNIT_SCOPE_OUTPUT: u32 = 2;

const K_AUDIO_UNIT_PROPERTY_STREAM_FORMAT: u32 = 8;
const K_AUDIO_UNIT_PROPERTY_MAXIMUM_FRAMES_PER_SLICE: u32 = 14;

const K_AUDIO_FORMAT_LINEAR_PCM: u32 = 0x6C70636D; // 'lpcm'
const K_AUDIO_FORMAT_FLAGS_NATIVE_FLOAT_PACKED: u32 = 0x00000001 | 0x00000004; // Float | Packed

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
    fn AudioUnitRender(
        unit: AudioUnit,
        io_action_flags: *mut u32,
        in_time_stamp: *const AudioTimeStamp,
        in_output_bus_number: u32,
        in_number_frames: u32,
        io_data: *mut AudioBufferList,
    ) -> OSStatus;
}

/// Audio Unit plugin instance
pub struct AuInstance {
    unit: AudioUnit,
    info: PluginInfo,
    sample_rate: f64,
    max_frames: u32,
    initialized: bool,
    // Audio buffers
    input_buffers: Vec<Vec<f32>>,
    output_buffers: Vec<Vec<f32>>,
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

        // Find the component
        let component = unsafe { AudioComponentFindNext(ptr::null_mut(), &au_desc) };
        if component.is_null() {
            return Err(Error::Plugin("Audio Unit not found".into()));
        }

        // Create instance
        let mut unit: AudioComponentInstance = ptr::null_mut();
        let status = unsafe { AudioComponentInstanceNew(component, &mut unit) };
        if status != 0 || unit.is_null() {
            return Err(Error::Plugin(format!("Failed to create AU instance: {}", status)));
        }

        let info = PluginInfo {
            id: format!("{:08X}{:08X}{:08X}", 
                desc.component_type, 
                desc.component_sub_type, 
                desc.component_manufacturer
            ),
            name: "Audio Unit".to_string(), // Would need AudioComponentCopyName
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
            input_buffers: vec![vec![0.0; 4096]; 2],
            output_buffers: vec![vec![0.0; 4096]; 2],
        })
    }

    /// Load an Audio Unit from a bundle path
    pub fn load_from_bundle(bundle_path: &Path) -> Result<Self> {
        // Parse the Info.plist to get component info
        // For now, return an error since we need more complex parsing
        Err(Error::Plugin("Bundle loading not yet implemented - use load() with AuDescription".into()))
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

        // Set input format
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

        // Set output format
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

        // Set max frames
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

        // Resize buffers
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

        // Copy input data
        for (ch, buf) in self.input_buffers.iter_mut().enumerate() {
            if let Some(channel) = buffer.channel(ch) {
                buf[..frames].copy_from_slice(&channel[..frames]);
            }
        }

        // Set up audio buffer list
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
            flags: 0x01, // kAudioTimeStampSampleTimeValid
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
            // Copy output data back
            for (ch, buf) in self.output_buffers.iter().enumerate() {
                if let Some(channel) = buffer.channel_mut(ch) {
                    channel[..frames].copy_from_slice(&buf[..frames]);
                }
            }
        }
    }

    fn set_parameter(&mut self, _id: u32, _value: f32) {
        // Would use AudioUnitSetParameter
    }

    fn get_parameter(&self, _id: u32) -> f32 {
        0.0
    }

    fn reset(&mut self) {
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
    fn test_au_description() {
        let desc = AuDescription::new(AuType::Effect, b"test", b"Test");
        assert_eq!(desc.component_type, AuType::Effect.os_type());
    }
}
