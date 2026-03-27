//! Pulse - Audio plugin framework
//!
//! Host VST3/AU/CLAP plugins or build custom audio effects.
//!
//! # Features
//!
//! - **Plugin Hosting**: Load and run VST3, AU, and CLAP plugins
//! - **Built-in Effects**: Reverb, delay, compressor, EQ, distortion
//! - **Plugin Development**: Trait-based API for building plugins
//! - **Real-time Safe**: Lock-free, no allocations in audio path
//!
//! # Quick Start
//!
//! ```ignore
//! use pulse::prelude::*;
//!
//! // Create a reverb effect
//! let mut reverb = Reverb::new(44100);
//! reverb.set_room_size(0.8);
//! reverb.set_damping(0.5);
//! reverb.set_wet(0.3);
//!
//! // Process audio
//! let mut buffer = AudioBuffer::new(2, 256);
//! reverb.process(&mut buffer, &ProcessContext::default());
//! ```

pub mod buffer;
pub mod cli;
pub mod effects;
pub mod format;
pub mod host;
pub mod param;
pub mod plugin;
pub mod preset;
pub mod process;

/// Prelude with common types
pub mod prelude {
    pub use crate::buffer::AudioBuffer;
    pub use crate::effects::{Compressor, Delay, Distortion, ParametricEQ, Reverb};
    pub use crate::param::{ParamInfo, ParamType, ParamValue, ParamSmoother};
    pub use crate::plugin::{Plugin, PluginCategory, PluginConfig, PluginInfo};
    pub use crate::process::{ProcessContext, TransportState};
    pub use crate::preset::Preset;
}

/// Error types
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Audio error: {0}")]
    Audio(String),
    #[error("Plugin error: {0}")]
    Plugin(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parameter error: {0}")]
    Parameter(String),
    #[error("Preset error: {0}")]
    Preset(String),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, Error>;
