//! VST3 plugin format support

mod host;
mod instance;

pub use host::Vst3Host;
pub use instance::Vst3Instance;

use std::path::{Path, PathBuf};

/// Get VST3 binary path within a bundle
pub fn get_vst3_binary_path(bundle_path: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let name = bundle_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("plugin");
        bundle_path.join("Contents").join("MacOS").join(name)
    }
    
    #[cfg(target_os = "windows")]
    {
        let name = bundle_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("plugin");
        // VST3 on Windows: bundle/Contents/x86_64-win/plugin.vst3
        bundle_path.join("Contents").join("x86_64-win").join(format!("{}.vst3", name))
    }
    
    #[cfg(target_os = "linux")]
    {
        let name = bundle_path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("plugin");
        // VST3 on Linux: bundle/Contents/x86_64-linux/plugin.so
        bundle_path.join("Contents").join("x86_64-linux").join(format!("{}.so", name))
    }
}

/// VST3 class IDs (GUIDs)
pub mod iid {
    /// IPluginFactory IID
    pub const IPLUGIN_FACTORY: [u8; 16] = [
        0x7A, 0x4D, 0x81, 0x1C, 0x52, 0x11, 0x45, 0x4F,
        0x86, 0xF9, 0x21, 0x66, 0x54, 0x18, 0x85, 0xF0
    ];
    
    /// IComponent IID
    pub const ICOMPONENT: [u8; 16] = [
        0xE8, 0x31, 0xFF, 0x31, 0xF2, 0xD5, 0x41, 0x01,
        0x92, 0x8E, 0xBB, 0xEE, 0x25, 0x69, 0x78, 0x02
    ];
    
    /// IAudioProcessor IID
    pub const IAUDIO_PROCESSOR: [u8; 16] = [
        0x42, 0x04, 0x3F, 0x99, 0xB7, 0xDA, 0x45, 0x3C,
        0xA5, 0x69, 0xE7, 0x9D, 0x9A, 0xAE, 0xC3, 0x3D
    ];
    
    /// IEditController IID
    pub const IEDIT_CONTROLLER: [u8; 16] = [
        0xDB, 0xA5, 0x13, 0x3A, 0xDA, 0x14, 0x41, 0x5B,
        0xAC, 0xDA, 0x13, 0x51, 0x82, 0x27, 0x78, 0x15
    ];
}
