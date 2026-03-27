//! CLAP plugin format support
//!
//! CLAP (CLever Audio Plugin) is a modern, open-source plugin format.
//! See https://cleveraudio.org for specification.

pub mod loader;

pub use loader::{ClapError, ClapLoader, ClapPluginInfo, default_clap_paths};
