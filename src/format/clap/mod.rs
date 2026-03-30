//! CLAP plugin format support

mod extensions;
mod host;
mod instance;
mod loader;

pub use extensions::{
    ClapParamInfo, ClapPluginParams, ClapPluginState, ClapPluginLatency, ClapPluginTail,
    ClapPluginAudioPorts, ClapPluginNotePorts, StateBuffer, ext_id, param_flags,
};
pub use host::ClapHost;
pub use instance::ClapInstance;
pub use loader::{ClapLoader, ClapPluginInfo, ClapError, default_clap_paths};
