//! Plugin traits and types

mod trait_def;
mod info;
mod config;

pub use trait_def::Plugin;
pub use info::{PluginInfo, PluginCategory};
pub use config::PluginConfig;
