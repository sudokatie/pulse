//! Plugin format support - VST3, AU, CLAP

pub mod au;
pub mod clap;
pub mod vst3;

pub use au::AuInstance;
pub use clap::{ClapInstance, ClapLoader, ClapHost};
pub use vst3::Vst3Instance;

use crate::plugin::Plugin;
use crate::Result;
use std::path::Path;

/// Load a plugin from a path, auto-detecting format
pub fn load_plugin(path: &Path) -> Result<Box<dyn Plugin>> {
    let extension = path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    
    match extension.to_lowercase().as_str() {
        "clap" => {
            // For CLAP, we need a plugin ID. Try to get info from the bundle.
            match ClapLoader::get_bundle_info(path) {
                Ok(info) => {
                    let instance = ClapInstance::load(path, &info.id)?;
                    Ok(Box::new(instance))
                }
                Err(_) => {
                    // Fallback: try loading with a generic ID
                    let id = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("plugin");
                    let instance = ClapInstance::load(path, id)?;
                    Ok(Box::new(instance))
                }
            }
        }
        "vst3" => {
            let instance = Vst3Instance::load(path, 0)?;
            Ok(Box::new(instance))
        }
        "component" => {
            #[cfg(target_os = "macos")]
            {
                let instance = AuInstance::load_from_bundle(path)?;
                Ok(Box::new(instance))
            }
            #[cfg(not(target_os = "macos"))]
            {
                Err(crate::Error::Plugin("Audio Units are only supported on macOS".into()))
            }
        }
        _ => Err(crate::Error::Plugin(format!("Unknown plugin format: {}", extension))),
    }
}
