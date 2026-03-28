//! Plugin database - cache scanned plugins with search

use std::path::{Path, PathBuf};
use std::fs;
use std::io;
use serde::{Deserialize, Serialize};
use crate::host::scanner::{PluginFormat, ScannedPlugin};

/// Plugin entry in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    /// Unique plugin ID
    pub id: String,
    /// Plugin name
    pub name: String,
    /// Full path to plugin
    pub path: PathBuf,
    /// Plugin format
    pub format: PluginFormat,
    /// Vendor name
    pub vendor: String,
    /// Category
    pub category: Option<String>,
    /// Number of audio inputs
    pub inputs: u32,
    /// Number of audio outputs
    pub outputs: u32,
    /// Whether plugin was successfully loaded
    pub verified: bool,
    /// Last scan timestamp (Unix epoch)
    pub last_scanned: u64,
}

impl PluginEntry {
    pub fn from_scanned(plugin: &ScannedPlugin) -> Self {
        let id = format!("{}.{}", 
            plugin.vendor.as_deref().unwrap_or("unknown").to_lowercase().replace(' ', "_"),
            plugin.name.to_lowercase().replace(' ', "_")
        );
        
        Self {
            id,
            name: plugin.name.clone(),
            path: plugin.path.clone(),
            format: plugin.format,
            vendor: plugin.vendor.clone().unwrap_or_else(|| "Unknown".to_string()),
            category: None,
            inputs: 2,
            outputs: 2,
            verified: false,
            last_scanned: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }
}

impl From<ScannedPlugin> for PluginEntry {
    fn from(plugin: ScannedPlugin) -> Self {
        PluginEntry::from_scanned(&plugin)
    }
}

/// Plugin database
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PluginDatabase {
    /// All plugins
    plugins: Vec<PluginEntry>,
    /// Database version
    pub version: u32,
}

impl PluginDatabase {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            version: 1,
        }
    }
    
    /// Load database from file
    pub fn load(path: &Path) -> io::Result<Self> {
        let contents = fs::read_to_string(path)?;
        serde_json::from_str(&contents)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
    
    /// Save database to file
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(path, contents)
    }
    
    /// Default database path
    pub fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("pulse")
            .join("plugins.json")
    }
    
    /// Add or update a plugin
    pub fn add_or_update(&mut self, entry: PluginEntry) {
        if let Some(existing) = self.plugins.iter_mut().find(|p| p.path == entry.path) {
            *existing = entry;
        } else {
            self.plugins.push(entry);
        }
    }
    
    /// Add plugins from scan results
    pub fn add_from_scan(&mut self, plugins: &[ScannedPlugin]) {
        for plugin in plugins {
            let exists = self.plugins.iter().any(|p| p.path == plugin.path);
            if !exists {
                self.plugins.push(PluginEntry::from_scanned(plugin));
            }
        }
    }
    
    /// Find plugin by ID
    pub fn find_by_id(&self, id: &str) -> Option<&PluginEntry> {
        self.plugins.iter().find(|p| p.id == id)
    }
    
    /// Get plugin by path
    pub fn get_by_path(&self, path: &Path) -> Option<&PluginEntry> {
        self.plugins.iter().find(|p| p.path == path)
    }
    
    /// Search by name (case-insensitive)
    pub fn search_by_name(&self, query: &str) -> impl Iterator<Item = &PluginEntry> {
        let query_lower = query.to_lowercase();
        self.plugins
            .iter()
            .filter(move |p| p.name.to_lowercase().contains(&query_lower))
    }
    
    /// Search by vendor (case-insensitive)
    pub fn search_by_vendor(&self, vendor: &str) -> impl Iterator<Item = &PluginEntry> {
        let vendor_lower = vendor.to_lowercase();
        self.plugins
            .iter()
            .filter(move |p| p.vendor.to_lowercase().contains(&vendor_lower))
    }
    
    /// Filter by format
    pub fn filter_by_format(&self, format: PluginFormat) -> impl Iterator<Item = &PluginEntry> {
        self.plugins.iter().filter(move |p| p.format == format)
    }
    
    /// Get all plugins
    pub fn all_plugins(&self) -> impl Iterator<Item = &PluginEntry> {
        self.plugins.iter()
    }
    
    /// Count plugins
    pub fn count(&self) -> usize {
        self.plugins.len()
    }
    
    /// Count by format
    pub fn count_by_format(&self, format: PluginFormat) -> usize {
        self.plugins.iter().filter(|p| p.format == format).count()
    }
    
    /// Remove plugins that no longer exist on disk
    pub fn prune(&mut self) -> usize {
        let before = self.plugins.len();
        self.plugins.retain(|p| p.path.exists());
        before - self.plugins.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    fn make_entry(name: &str, format: PluginFormat) -> PluginEntry {
        PluginEntry {
            id: format!("test.{}", name.to_lowercase()),
            name: name.to_string(),
            path: PathBuf::from(format!("/test/{}.{}", name, format.extension())),
            format,
            vendor: "Test".to_string(),
            category: None,
            inputs: 2,
            outputs: 2,
            verified: false,
            last_scanned: 0,
        }
    }
    
    #[test]
    fn test_database_new() {
        let db = PluginDatabase::new();
        assert_eq!(db.count(), 0);
        assert_eq!(db.version, 1);
    }
    
    #[test]
    fn test_database_save_load() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("plugins.json");
        
        let mut db = PluginDatabase::new();
        db.add_or_update(make_entry("Reverb", PluginFormat::Vst3));
        db.add_or_update(make_entry("Compressor", PluginFormat::Clap));
        db.save(&path).unwrap();
        
        let loaded = PluginDatabase::load(&path).unwrap();
        assert_eq!(loaded.count(), 2);
    }
    
    #[test]
    fn test_database_search_by_name() {
        let mut db = PluginDatabase::new();
        db.add_or_update(make_entry("FabFilter Pro-Q", PluginFormat::Vst3));
        db.add_or_update(make_entry("FabFilter Pro-C", PluginFormat::Vst3));
        db.add_or_update(make_entry("Ozone", PluginFormat::Vst3));
        
        let results: Vec<_> = db.search_by_name("fabfilter").collect();
        assert_eq!(results.len(), 2);
        
        let results: Vec<_> = db.search_by_name("pro-q").collect();
        assert_eq!(results.len(), 1);
    }
    
    #[test]
    fn test_database_search_by_vendor() {
        let mut db = PluginDatabase::new();
        
        let mut entry = make_entry("Pro-Q", PluginFormat::Vst3);
        entry.vendor = "FabFilter".to_string();
        db.add_or_update(entry);
        
        let mut entry = make_entry("Ozone", PluginFormat::Vst3);
        entry.vendor = "iZotope".to_string();
        db.add_or_update(entry);
        
        let results: Vec<_> = db.search_by_vendor("fabfilter").collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Pro-Q");
    }
    
    #[test]
    fn test_database_filter_by_format() {
        let mut db = PluginDatabase::new();
        db.add_or_update(make_entry("Plugin1", PluginFormat::Vst3));
        db.add_or_update(make_entry("Plugin2", PluginFormat::Vst3));
        db.add_or_update(make_entry("Plugin3", PluginFormat::Clap));
        
        let results: Vec<_> = db.filter_by_format(PluginFormat::Vst3).collect();
        assert_eq!(results.len(), 2);
        
        let results: Vec<_> = db.filter_by_format(PluginFormat::Clap).collect();
        assert_eq!(results.len(), 1);
    }
    
    #[test]
    fn test_database_upsert() {
        let mut db = PluginDatabase::new();
        
        let mut entry = make_entry("Plugin", PluginFormat::Vst3);
        entry.verified = false;
        db.add_or_update(entry);
        assert_eq!(db.count(), 1);
        
        let mut entry = make_entry("Plugin", PluginFormat::Vst3);
        entry.verified = true;
        db.add_or_update(entry);
        
        assert_eq!(db.count(), 1);
        assert!(db.plugins[0].verified);
    }
    
    #[test]
    fn test_database_count_by_format() {
        let mut db = PluginDatabase::new();
        db.add_or_update(make_entry("P1", PluginFormat::Vst3));
        db.add_or_update(make_entry("P2", PluginFormat::Vst3));
        db.add_or_update(make_entry("P3", PluginFormat::Clap));
        
        assert_eq!(db.count_by_format(PluginFormat::Vst3), 2);
        assert_eq!(db.count_by_format(PluginFormat::Clap), 1);
        assert_eq!(db.count_by_format(PluginFormat::AudioUnit), 0);
    }
    
    #[test]
    fn test_plugin_entry_format_conversion() {
        let entry = make_entry("Test", PluginFormat::Vst3);
        assert_eq!(entry.format, PluginFormat::Vst3);
    }
}
