//! Audio Unit plugin format support (macOS only)

#[cfg(target_os = "macos")]
mod instance;

#[cfg(target_os = "macos")]
pub use instance::AuInstance;

#[cfg(not(target_os = "macos"))]
mod stub;

#[cfg(not(target_os = "macos"))]
pub use stub::AuInstance;

use std::path::{Path, PathBuf};

/// Audio Unit component types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AuType {
    Effect,
    MusicDevice,
    MusicEffect,
    Mixer,
    Generator,
    Panner,
    OfflineEffect,
}

impl AuType {
    /// Get the OSType for this AU type
    pub fn os_type(&self) -> u32 {
        match self {
            AuType::Effect => fourcc(b"aufx"),
            AuType::MusicDevice => fourcc(b"aumu"),
            AuType::MusicEffect => fourcc(b"aumf"),
            AuType::Mixer => fourcc(b"aumx"),
            AuType::Generator => fourcc(b"augn"),
            AuType::Panner => fourcc(b"aupn"),
            AuType::OfflineEffect => fourcc(b"auol"),
        }
    }
}

/// Convert 4 bytes to a FourCC u32
fn fourcc(bytes: &[u8; 4]) -> u32 {
    ((bytes[0] as u32) << 24)
        | ((bytes[1] as u32) << 16)
        | ((bytes[2] as u32) << 8)
        | (bytes[3] as u32)
}

/// Get AU binary path within a bundle
pub fn get_au_binary_path(bundle_path: &Path) -> PathBuf {
    let name = bundle_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("plugin");
    bundle_path.join("Contents").join("MacOS").join(name)
}

/// Audio Unit component description
#[derive(Debug, Clone)]
pub struct AuDescription {
    pub component_type: u32,
    pub component_sub_type: u32,
    pub component_manufacturer: u32,
}

impl AuDescription {
    pub fn new(au_type: AuType, sub_type: &[u8; 4], manufacturer: &[u8; 4]) -> Self {
        Self {
            component_type: au_type.os_type(),
            component_sub_type: fourcc(sub_type),
            component_manufacturer: fourcc(manufacturer),
        }
    }
}
