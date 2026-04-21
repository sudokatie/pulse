//! CLI commands

pub mod effect;
pub mod install;
pub mod package;
pub mod plugins;

pub use effect::{EffectType, ProcessResult, list_effects, process_effect};
pub use install::{InstallResult, install_bundle, validate_bundle, print_install_result, print_validation_result};
pub use package::{PackageOptions, PackageResult, build_package, parse_platform, print_result};
pub use plugins::{ScanResult, PluginInfo, DetailedPluginInfo, scan_plugins, list_plugins, get_plugin_info, load_database, save_database};
