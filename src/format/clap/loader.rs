//! CLAP plugin loader
//!
//! Loads CLAP plugins from .clap bundles.

use std::path::{Path, PathBuf};
use std::fs;

/// CLAP plugin info extracted from bundle
#[derive(Debug, Clone)]
pub struct ClapPluginInfo {
    /// Plugin ID (reverse domain, e.g., "com.vendor.plugin")
    pub id: String,
    /// Display name
    pub name: String,
    /// Vendor name
    pub vendor: String,
    /// Version string
    pub version: String,
    /// Description
    pub description: String,
    /// Path to the bundle
    pub path: PathBuf,
    /// Features (e.g., "audio-effect", "instrument")
    pub features: Vec<String>,
}

impl ClapPluginInfo {
    /// Check if plugin is an instrument
    pub fn is_instrument(&self) -> bool {
        self.features.iter().any(|f| f == "instrument")
    }
    
    /// Check if plugin is an effect
    pub fn is_effect(&self) -> bool {
        self.features.iter().any(|f| f == "audio-effect")
    }
    
    /// Check if plugin is an analyzer
    pub fn is_analyzer(&self) -> bool {
        self.features.iter().any(|f| f == "analyzer")
    }
}

/// Error type for CLAP loading
#[derive(Debug, thiserror::Error)]
pub enum ClapError {
    #[error("Bundle not found: {0}")]
    BundleNotFound(PathBuf),
    
    #[error("Invalid bundle structure: {0}")]
    InvalidBundle(String),
    
    #[error("Failed to load plugin: {0}")]
    LoadFailed(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Plugin not found in bundle: {0}")]
    PluginNotFound(String),
}

/// CLAP plugin loader
pub struct ClapLoader {
    /// Search paths for CLAP plugins
    search_paths: Vec<PathBuf>,
}

impl ClapLoader {
    pub fn new() -> Self {
        Self {
            search_paths: default_clap_paths(),
        }
    }
    
    pub fn with_paths(paths: Vec<PathBuf>) -> Self {
        Self { search_paths: paths }
    }
    
    /// Add a search path
    pub fn add_path(&mut self, path: impl Into<PathBuf>) {
        self.search_paths.push(path.into());
    }
    
    /// Get bundle binary path (platform-specific)
    pub fn get_binary_path(bundle_path: &Path) -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            let binary = bundle_path.join("Contents/MacOS");
            if binary.exists() {
                // Find the actual binary inside
                if let Ok(entries) = fs::read_dir(&binary) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            return Some(path);
                        }
                    }
                }
            }
            None
        }
        
        #[cfg(target_os = "windows")]
        {
            // On Windows, .clap is a DLL directly
            if bundle_path.extension().map(|e| e == "clap").unwrap_or(false) {
                return Some(bundle_path.to_path_buf());
            }
            None
        }
        
        #[cfg(target_os = "linux")]
        {
            // On Linux, .clap is an SO directly
            if bundle_path.extension().map(|e| e == "clap").unwrap_or(false) {
                return Some(bundle_path.to_path_buf());
            }
            None
        }
        
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            None
        }
    }
    
    /// Check if a path is a valid CLAP bundle
    pub fn is_valid_bundle(path: &Path) -> bool {
        if !path.exists() {
            return false;
        }
        
        let ext = path.extension().and_then(|e| e.to_str());
        if ext != Some("clap") {
            return false;
        }
        
        // On macOS, check for bundle structure
        #[cfg(target_os = "macos")]
        {
            return path.join("Contents/MacOS").exists();
        }
        
        // On other platforms, the .clap file is the binary
        #[cfg(not(target_os = "macos"))]
        {
            return path.is_file();
        }
    }
    
    /// Scan for all CLAP plugins
    pub fn scan(&self) -> Vec<PathBuf> {
        let mut plugins = Vec::new();
        
        for search_path in &self.search_paths {
            if search_path.exists() {
                self.scan_directory(search_path, &mut plugins);
            }
        }
        
        plugins
    }
    
    fn scan_directory(&self, dir: &Path, plugins: &mut Vec<PathBuf>) {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        
        for entry in entries.flatten() {
            let path = entry.path();
            
            if Self::is_valid_bundle(&path) {
                plugins.push(path);
            } else if path.is_dir() {
                self.scan_directory(&path, plugins);
            }
        }
    }
    
    /// Get basic info from a bundle path (without loading the binary)
    pub fn get_bundle_info(path: &Path) -> Result<ClapPluginInfo, ClapError> {
        if !path.exists() {
            return Err(ClapError::BundleNotFound(path.to_path_buf()));
        }
        
        if !Self::is_valid_bundle(path) {
            return Err(ClapError::InvalidBundle("Not a valid CLAP bundle".into()));
        }
        
        // Extract name from bundle filename
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();
        
        // Try to read Info.plist on macOS for more details
        #[cfg(target_os = "macos")]
        let (vendor, version, description) = {
            let plist_path = path.join("Contents/Info.plist");
            if plist_path.exists() {
                // Basic plist parsing - in production would use plist crate
                ("Unknown".to_string(), "1.0.0".to_string(), String::new())
            } else {
                ("Unknown".to_string(), "1.0.0".to_string(), String::new())
            }
        };
        
        #[cfg(not(target_os = "macos"))]
        let (vendor, version, description) = {
            ("Unknown".to_string(), "1.0.0".to_string(), String::new())
        };
        
        Ok(ClapPluginInfo {
            id: format!("com.unknown.{}", name.to_lowercase().replace(' ', "-")),
            name,
            vendor,
            version,
            description,
            path: path.to_path_buf(),
            features: vec!["audio-effect".to_string()],
        })
    }
}

impl Default for ClapLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Default CLAP search paths
pub fn default_clap_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    
    #[cfg(target_os = "macos")]
    {
        paths.push(PathBuf::from("/Library/Audio/Plug-Ins/CLAP"));
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join("Library/Audio/Plug-Ins/CLAP"));
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        paths.push(PathBuf::from("C:\\Program Files\\Common Files\\CLAP"));
        if let Some(local) = dirs::data_local_dir() {
            paths.push(local.join("Programs\\Common\\CLAP"));
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        paths.push(PathBuf::from("/usr/lib/clap"));
        paths.push(PathBuf::from("/usr/local/lib/clap"));
        if let Some(home) = dirs::home_dir() {
            paths.push(home.join(".clap"));
        }
    }
    
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_clap_plugin_info_is_instrument() {
        let info = ClapPluginInfo {
            id: "com.test.synth".to_string(),
            name: "TestSynth".to_string(),
            vendor: "Test".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            path: PathBuf::from("/test/synth.clap"),
            features: vec!["instrument".to_string()],
        };
        assert!(info.is_instrument());
        assert!(!info.is_effect());
    }
    
    #[test]
    fn test_clap_plugin_info_is_effect() {
        let info = ClapPluginInfo {
            id: "com.test.reverb".to_string(),
            name: "TestReverb".to_string(),
            vendor: "Test".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            path: PathBuf::from("/test/reverb.clap"),
            features: vec!["audio-effect".to_string()],
        };
        assert!(info.is_effect());
        assert!(!info.is_instrument());
    }
    
    #[test]
    fn test_clap_loader_new() {
        let loader = ClapLoader::new();
        assert!(!loader.search_paths.is_empty());
    }
    
    #[test]
    fn test_clap_loader_with_paths() {
        let paths = vec![PathBuf::from("/custom/path")];
        let loader = ClapLoader::with_paths(paths.clone());
        assert_eq!(loader.search_paths, paths);
    }
    
    #[test]
    fn test_default_clap_paths() {
        let paths = default_clap_paths();
        assert!(!paths.is_empty());
    }
    
    #[test]
    #[cfg(not(target_os = "macos"))]
    fn test_is_valid_bundle_file() {
        let temp = TempDir::new().unwrap();
        let clap_file = temp.path().join("test.clap");
        fs::write(&clap_file, b"mock clap data").unwrap();
        
        assert!(ClapLoader::is_valid_bundle(&clap_file));
    }
    
    #[test]
    fn test_is_valid_bundle_wrong_extension() {
        let temp = TempDir::new().unwrap();
        let wrong_file = temp.path().join("test.vst3");
        fs::create_dir(&wrong_file).unwrap();
        
        assert!(!ClapLoader::is_valid_bundle(&wrong_file));
    }
    
    #[test]
    fn test_scan_empty_directory() {
        let temp = TempDir::new().unwrap();
        let loader = ClapLoader::with_paths(vec![temp.path().to_path_buf()]);
        let plugins = loader.scan();
        assert!(plugins.is_empty());
    }
}
