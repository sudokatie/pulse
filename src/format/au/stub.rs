//! Audio Unit stub for non-macOS platforms

use super::AuDescription;
use crate::buffer::AudioBuffer;
use crate::plugin::{Plugin, PluginCategory, PluginConfig, PluginInfo};
use crate::process::ProcessContext;
use crate::{Error, Result};
use std::path::Path;

/// Stub Audio Unit instance for non-macOS platforms
pub struct AuInstance {
    info: PluginInfo,
}

impl AuInstance {
    /// Audio Units are only supported on macOS
    pub fn load(_desc: &AuDescription) -> Result<Self> {
        Err(Error::Plugin("Audio Units are only supported on macOS".into()))
    }

    /// Audio Units are only supported on macOS
    pub fn load_from_bundle(_bundle_path: &Path) -> Result<Self> {
        Err(Error::Plugin("Audio Units are only supported on macOS".into()))
    }
}

impl Plugin for AuInstance {
    fn info(&self) -> PluginInfo {
        self.info.clone()
    }

    fn init(&mut self, _config: &PluginConfig) -> Result<()> {
        Err(Error::Plugin("Audio Units are only supported on macOS".into()))
    }

    fn process(&mut self, _buffer: &mut AudioBuffer, _ctx: &ProcessContext) {
        // No-op
    }

    fn set_parameter(&mut self, _id: u32, _value: f32) {}

    fn get_parameter(&self, _id: u32) -> f32 {
        0.0
    }

    fn reset(&mut self) {}
}
