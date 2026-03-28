//! Plugin scanner - discovers VST3/AU/CLAP plugins on the system

use std::path::{Path, PathBuf};
use std::fs;
use serde::{Deserialize, Serialize};

/// Plugin format types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginFormat {
    Vst3,
    AudioUnit,
    Clap,
}

impl PluginFormat {
    /// File extension for this format
    pub fn extension(&self) -> &'static str {
        match self {
            PluginFormat::Vst3 => "vst3",
            PluginFormat::AudioUnit => "component",
            PluginFormat::Clap => "clap",
        }
    }
    
    /// Display name
    pub fn name(&self) -> &'static str {
        match self {
            PluginFormat::Vst3 => "VST3",
            PluginFormat::AudioUnit => "Audio Unit",
            PluginFormat::Clap => "CLAP",
        }
    }
}

/// Scanned plugin info (basic metadata without loading)
#[derive(Debug, Clone)]
pub struct ScannedPlugin {
    /// Plugin name (from bundle name)
    pub name: String,
    /// Full path to plugin bundle
    pub path: PathBuf,
    /// Plugin format
    pub format: PluginFormat,
    /// Vendor (from path, if detectable)
    pub vendor: Option<String>,
}

impl ScannedPlugin {
    pub fn new(path: PathBuf, format: PluginFormat) -> Self {
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();
        
        Self {
            name,
            path,
            format,
            vendor: None,
        }
    }
    
    pub fn with_vendor(mut self, vendor: impl Into<String>) -> Self {
        self.vendor = Some(vendor.into());
        self
    }
}

/// Plugin scanner configuration
#[derive(Debug, Clone)]
pub struct ScannerConfig {
    /// Search paths for plugins
    pub search_paths: Vec<PathBuf>,
    /// Formats to scan for
    pub formats: Vec<PluginFormat>,
    /// Whether to follow symlinks
    pub follow_symlinks: bool,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            search_paths: default_search_paths(),
            formats: vec![PluginFormat::Vst3, PluginFormat::AudioUnit, PluginFormat::Clap],
            follow_symlinks: true,
        }
    }
}

/// Get default plugin search paths for the current platform
pub fn default_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    
    #[cfg(target_os = "macos")]
    {
        // System-wide
        paths.push(PathBuf::from("/Library/Audio/Plug-Ins/VST3"));
        paths.push(PathBuf::from("/Library/Audio/Plug-Ins/Components"));
        paths.push(PathBuf::from("/Library/Audio/Plug-Ins/CLAP"));
        
        // User-specific
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join("Library/Audio/Plug-Ins/VST3"));
            paths.push(home.join("Library/Audio/Plug-Ins/Components"));
            paths.push(home.join("Library/Audio/Plug-Ins/CLAP"));
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        // Common paths
        paths.push(PathBuf::from("C:\\Program Files\\Common Files\\VST3"));
        paths.push(PathBuf::from("C:\\Program Files\\Common Files\\CLAP"));
        
        // User-specific
        if let Some(local) = dirs::data_local_dir() {
            paths.push(local.join("Programs\\Common\\VST3"));
            paths.push(local.join("Programs\\Common\\CLAP"));
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        // System-wide
        paths.push(PathBuf::from("/usr/lib/vst3"));
        paths.push(PathBuf::from("/usr/lib/clap"));
        paths.push(PathBuf::from("/usr/local/lib/vst3"));
        paths.push(PathBuf::from("/usr/local/lib/clap"));
        
        // User-specific
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join(".vst3"));
            paths.push(home.join(".clap"));
        }
    }
    
    paths
}

/// Plugin scanner
pub struct PluginScanner {
    config: ScannerConfig,
}

impl PluginScanner {
    pub fn new() -> Self {
        Self {
            config: ScannerConfig::default(),
        }
    }
    
    pub fn with_config(config: ScannerConfig) -> Self {
        Self { config }
    }
    
    /// Add a custom search path
    pub fn add_search_path(&mut self, path: impl Into<PathBuf>) {
        self.config.search_paths.push(path.into());
    }
    
    /// Scan all configured paths for plugins
    pub fn scan(&self) -> Vec<ScannedPlugin> {
        let mut plugins = Vec::new();
        
        for path in &self.config.search_paths {
            if path.exists() {
                self.scan_directory(path, &mut plugins);
            }
        }
        
        // Sort by name
        plugins.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        plugins
    }
    
    /// Alias for scan()
    pub fn scan_all(&self) -> Vec<ScannedPlugin> {
        self.scan()
    }
    
    /// Scan a single directory
    pub fn scan_directory(&self, dir: &Path, plugins: &mut Vec<ScannedPlugin>) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        
        for entry in entries.flatten() {
            let path = entry.path();
            
            // Check if it's a plugin bundle
            if let Some(format) = self.detect_format(&path) {
                if self.config.formats.contains(&format) {
                    plugins.push(ScannedPlugin::new(path, format));
                }
            } else if path.is_dir() {
                // Recurse into subdirectories (for vendor folders)
                self.scan_directory(&path, plugins);
            }
        }
    }
    
    /// Detect plugin format from path
    fn detect_format(&self, path: &Path) -> Option<PluginFormat> {
        let ext = path.extension()?.to_str()?;
        
        match ext.to_lowercase().as_str() {
            "vst3" => Some(PluginFormat::Vst3),
            "component" => Some(PluginFormat::AudioUnit),
            "clap" => Some(PluginFormat::Clap),
            _ => None,
        }
    }
    
    /// Scan a specific path (file or directory)
    pub fn scan_path(&self, path: &Path) -> Vec<ScannedPlugin> {
        let mut plugins = Vec::new();
        
        if path.is_file() || self.detect_format(path).is_some() {
            if let Some(format) = self.detect_format(path) {
                if self.config.formats.contains(&format) {
                    plugins.push(ScannedPlugin::new(path.to_path_buf(), format));
                }
            }
        } else if path.is_dir() {
            self.scan_directory(path, &mut plugins);
        }
        
        plugins
    }
}

impl Default for PluginScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    
    #[test]
    fn test_plugin_format_extension() {
        assert_eq!(PluginFormat::Vst3.extension(), "vst3");
        assert_eq!(PluginFormat::AudioUnit.extension(), "component");
        assert_eq!(PluginFormat::Clap.extension(), "clap");
    }
    
    #[test]
    fn test_plugin_format_name() {
        assert_eq!(PluginFormat::Vst3.name(), "VST3");
        assert_eq!(PluginFormat::AudioUnit.name(), "Audio Unit");
        assert_eq!(PluginFormat::Clap.name(), "CLAP");
    }
    
    #[test]
    fn test_scanned_plugin_new() {
        let path = PathBuf::from("/test/MyPlugin.vst3");
        let plugin = ScannedPlugin::new(path.clone(), PluginFormat::Vst3);
        
        assert_eq!(plugin.name, "MyPlugin");
        assert_eq!(plugin.path, path);
        assert_eq!(plugin.format, PluginFormat::Vst3);
        assert!(plugin.vendor.is_none());
    }
    
    #[test]
    fn test_scanned_plugin_with_vendor() {
        let path = PathBuf::from("/test/MyPlugin.clap");
        let plugin = ScannedPlugin::new(path, PluginFormat::Clap)
            .with_vendor("TestVendor");
        
        assert_eq!(plugin.vendor, Some("TestVendor".to_string()));
    }
    
    #[test]
    fn test_scanner_scan_directory() {
        let temp = TempDir::new().unwrap();
        
        // Create mock plugin bundles (just directories with right extensions)
        fs::create_dir(temp.path().join("Plugin1.vst3")).unwrap();
        fs::create_dir(temp.path().join("Plugin2.clap")).unwrap();
        fs::create_dir(temp.path().join("NotAPlugin.txt")).unwrap();
        
        let mut config = ScannerConfig::default();
        config.search_paths = vec![temp.path().to_path_buf()];
        
        let scanner = PluginScanner::with_config(config);
        let plugins = scanner.scan();
        
        assert_eq!(plugins.len(), 2);
        assert!(plugins.iter().any(|p| p.name == "Plugin1"));
        assert!(plugins.iter().any(|p| p.name == "Plugin2"));
    }
    
    #[test]
    fn test_scanner_format_filter() {
        let temp = TempDir::new().unwrap();
        
        fs::create_dir(temp.path().join("Plugin1.vst3")).unwrap();
        fs::create_dir(temp.path().join("Plugin2.clap")).unwrap();
        
        let mut config = ScannerConfig::default();
        config.search_paths = vec![temp.path().to_path_buf()];
        config.formats = vec![PluginFormat::Vst3]; // Only VST3
        
        let scanner = PluginScanner::with_config(config);
        let plugins = scanner.scan();
        
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].format, PluginFormat::Vst3);
    }
    
    #[test]
    fn test_scanner_nested_directories() {
        let temp = TempDir::new().unwrap();
        
        // Create vendor subfolder
        let vendor_dir = temp.path().join("Steinberg");
        fs::create_dir(&vendor_dir).unwrap();
        fs::create_dir(vendor_dir.join("Cubase.vst3")).unwrap();
        
        let mut config = ScannerConfig::default();
        config.search_paths = vec![temp.path().to_path_buf()];
        
        let scanner = PluginScanner::with_config(config);
        let plugins = scanner.scan();
        
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "Cubase");
    }
    
    #[test]
    fn test_default_search_paths() {
        let paths = default_search_paths();
        // Should have at least some paths on any platform
        assert!(!paths.is_empty());
    }
}
