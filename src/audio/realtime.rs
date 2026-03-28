//! Real-time audio I/O using cpal

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, Stream, StreamConfig, SampleFormat};
use parking_lot::Mutex;
use std::sync::Arc;
use crate::Result;

/// List available audio output devices
pub fn list_audio_devices() -> Result<Vec<String>> {
    let host = cpal::default_host();
    let devices: Vec<String> = host.output_devices()
        .map_err(|e| crate::Error::Audio(format!("Failed to enumerate devices: {}", e)))?
        .filter_map(|d| d.name().ok())
        .collect();
    Ok(devices)
}

/// Get default output device name
pub fn default_device_name() -> Result<String> {
    let host = cpal::default_host();
    host.default_output_device()
        .and_then(|d| d.name().ok())
        .ok_or_else(|| crate::Error::Audio("No default output device".into()))
}

/// Audio device wrapper
pub struct AudioDevice {
    device: Device,
    config: StreamConfig,
}

impl AudioDevice {
    /// Open default output device
    pub fn open_default() -> Result<Self> {
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or_else(|| crate::Error::Audio("No output device available".into()))?;
        
        let config = device.default_output_config()
            .map_err(|e| crate::Error::Audio(format!("Failed to get config: {}", e)))?;
        
        Ok(Self {
            device,
            config: config.into(),
        })
    }

    /// Open device by name
    pub fn open_by_name(name: &str) -> Result<Self> {
        let host = cpal::default_host();
        let device = host.output_devices()
            .map_err(|e| crate::Error::Audio(format!("Failed to enumerate: {}", e)))?
            .find(|d| d.name().map(|n| n.contains(name)).unwrap_or(false))
            .ok_or_else(|| crate::Error::Audio(format!("Device not found: {}", name)))?;
        
        let config = device.default_output_config()
            .map_err(|e| crate::Error::Audio(format!("Failed to get config: {}", e)))?;
        
        Ok(Self {
            device,
            config: config.into(),
        })
    }

    /// Get sample rate
    pub fn sample_rate(&self) -> u32 {
        self.config.sample_rate.0
    }

    /// Get channel count
    pub fn channels(&self) -> u16 {
        self.config.channels
    }
}

/// Audio stream for real-time playback
pub struct AudioStream {
    _stream: Stream,
    running: Arc<Mutex<bool>>,
}

impl AudioStream {
    /// Create stream with callback
    pub fn new<F>(device: &AudioDevice, mut callback: F) -> Result<Self>
    where
        F: FnMut(&mut [f32]) + Send + 'static,
    {
        let running = Arc::new(Mutex::new(true));
        let running_clone = running.clone();

        let stream = device.device.build_output_stream(
            &device.config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                if *running_clone.lock() {
                    callback(data);
                } else {
                    data.fill(0.0);
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        ).map_err(|e| crate::Error::Audio(format!("Failed to build stream: {}", e)))?;

        stream.play()
            .map_err(|e| crate::Error::Audio(format!("Failed to start stream: {}", e)))?;

        Ok(Self {
            _stream: stream,
            running,
        })
    }

    /// Stop the stream
    pub fn stop(&self) {
        *self.running.lock() = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_devices() {
        // Should not panic
        let _ = list_audio_devices();
    }

    #[test]
    fn test_default_device_name() {
        // May fail if no audio device
        let _ = default_device_name();
    }
}
