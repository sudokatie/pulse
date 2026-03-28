//! CLAP plugin format support

mod loader;
mod host;
mod instance;

pub use loader::{ClapLoader, ClapPluginInfo, ClapError, default_clap_paths};
pub use host::ClapHost;
pub use instance::ClapInstance;
