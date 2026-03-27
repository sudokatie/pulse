//! CLI commands

pub mod effect;
pub mod plugins;

pub use effect::{EffectType, ProcessResult, list_effects, process_effect};
pub use plugins::{ScanResult, PluginInfo, DetailedPluginInfo, scan_plugins, list_plugins, get_plugin_info, load_database, save_database};
