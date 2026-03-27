//! Plugin hosting - scanner, database, and plugin instance management

pub mod database;
pub mod scanner;

pub use database::{PluginDatabase, PluginEntry};
pub use scanner::{PluginFormat, PluginScanner, ScannedPlugin, ScannerConfig, default_search_paths};
