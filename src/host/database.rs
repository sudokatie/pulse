//! Plugin database - cache scanned plugins with search

use std::path::{Path, PathBuf};
use std::fs;
use std::io;
use serde::{Deserialize, Serialize};
use crate::host::scanner::{PluginFormat, ScannedPlugin};

/// Plugin entry in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEntry {
    /// Plugin name
    pub name: String,
    /// Full path to plugin
    pub path: String,
    /// Plugin format (stored as string for JSON)
    pub format: String,
    /// Vendor name
    pub vendor: Option<String>,
    /// Category
    pub category: Option<String>,
    /// Whether plugin was successfully loaded
    pub verified: bool,
    /// Last scan timestamp (Unix epoch)
    pub last_scanned: u64,
}

impl PluginEntry {
    pub fn from_scanned(plugin: &ScannedPlugin) -> Self {
        Self {
            name: plugin.name.clone(),
            path: plugin.path.to_string_lossy().to_string(),
            format: plugin.format.name().to_string(),
            vendor: plugin.vendor.clone(),
            category: None,
            verified: false,
            last_scanned: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }
    
    pub fn format(&self) -> Option<PluginFormat> {
        match self.format.as_str() {
            "VST3" => Some(PluginFormat::Vst3),
            "Audio Unit" => Some(PluginFormat::AudioUnit),
            "CLAP" => Some(PluginFormat::Clap),
            _ => None,
        }
    }
}

/// Plugin database
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PluginDatabase {
    /// All plugins
    pub plugins: Vec<PluginEntry>,
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
        // Create parent directory if needed
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
    
    /// Add plugins from scan results
    pub fn add_from_scan(&mut self, plugins: &[ScannedPlugin]) {
        for plugin in plugins {
            // Check if already exists
            let exists = self.plugins.iter().any(|p| p.path == plugin.path.to_string_lossy());
            
            if !exists {
                self.plugins.push(PluginEntry::from_scanned(plugin));
            }
        }
    }
    
    /// Update or add a plugin
    pub fn upsert(&mut self, entry: PluginEntry) {
        if let Some(existing) = self.plugins.iter_mut().find(|p| p.path == entry.path) {
            *existing = entry;
        } else {
            self.plugins.push(entry);
        }
    }
    
    /// Remove plugins that no longer exist on disk
    pub fn prune(&mut self) -> usize {
        let before = self.plugins.len();
        self.plugins.retain(|p| Path::new(&p.path).exists());
        before - self.plugins.len()
    }
    
    /// Search by name (case-insensitive)
    pub fn search_by_name(&self, query: &str) -> Vec<&PluginEntry> {
        let query_lower = query.to_lowercase();
        self.plugins
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&query_lower))
            .collect()
    }
    
    /// Search by vendor (case-insensitive)
    pub fn search_by_vendor(&self, vendor: &str) -> Vec<&PluginEntry> {
        let vendor_lower = vendor.to_lowercase();
        self.plugins
            .iter()
            .filter(|p| {
                p.vendor
                    .as_ref()
                    .map(|v| v.to_lowercase().contains(&vendor_lower))
                    .unwrap_or(false)
            })
            .collect()
    }
    
    /// Filter by format
    pub fn filter_by_format(&self, format: PluginFormat) -> Vec<&PluginEntry> {
        self.plugins
            .iter()
            .filter(|p| p.format == format.name())
            .collect()
    }
    
    /// Filter by category
    pub fn filter_by_category(&self, category: &str) -> Vec<&PluginEntry> {
        let category_lower = category.to_lowercase();
        self.plugins
            .iter()
            .filter(|p| {
                p.category
                    .as_ref()
                    .map(|c| c.to_lowercase() == category_lower)
                    .unwrap_or(false)
            })
            .collect()
    }
    
    /// Get plugin by path
    pub fn get_by_path(&self, path: &str) -> Option<&PluginEntry> {
        self.plugins.iter().find(|p| p.path == path)
    }
    
    /// Get all plugins
    pub fn all(&self) -> &[PluginEntry] {
        &self.plugins
    }
    
    /// Count plugins
    pub fn count(&self) -> usize {
        self.plugins.len()
    }
    
    /// Count by format
    pub fn count_by_format(&self, format: PluginFormat) -> usize {
        self.plugins.iter().filter(|p| p.format == format.name()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    fn make_entry(name: &str, format: &str) -> PluginEntry {
        PluginEntry {
            name: name.to_string(),
            path: format!("/test/{}.{}", name, format.to_lowercase()),
            format: format.to_string(),
            vendor: None,
            category: None,
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
        db.upsert(make_entry("Reverb", "VST3"));
        db.upsert(make_entry("Compressor", "CLAP"));
        db.save(&path).unwrap();
        
        let loaded = PluginDatabase::load(&path).unwrap();
        assert_eq!(loaded.count(), 2);
    }
    
    #[test]
    fn test_database_search_by_name() {
        let mut db = PluginDatabase::new();
        db.upsert(make_entry("FabFilter Pro-Q", "VST3"));
        db.upsert(make_entry("FabFilter Pro-C", "VST3"));
        db.upsert(make_entry("Ozone", "VST3"));
        
        let results = db.search_by_name("fabfilter");
        assert_eq!(results.len(), 2);
        
        let results = db.search_by_name("pro-q");
        assert_eq!(results.len(), 1);
    }
    
    #[test]
    fn test_database_search_by_vendor() {
        let mut db = PluginDatabase::new();
        
        let mut entry = make_entry("Pro-Q", "VST3");
        entry.vendor = Some("FabFilter".to_string());
        db.upsert(entry);
        
        let mut entry = make_entry("Ozone", "VST3");
        entry.vendor = Some("iZotope".to_string());
        db.upsert(entry);
        
        let results = db.search_by_vendor("fabfilter");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Pro-Q");
    }
    
    #[test]
    fn test_database_filter_by_format() {
        let mut db = PluginDatabase::new();
        db.upsert(make_entry("Plugin1", "VST3"));
        db.upsert(make_entry("Plugin2", "VST3"));
        db.upsert(make_entry("Plugin3", "CLAP"));
        
        let results = db.filter_by_format(PluginFormat::Vst3);
        assert_eq!(results.len(), 2);
        
        let results = db.filter_by_format(PluginFormat::Clap);
        assert_eq!(results.len(), 1);
    }
    
    #[test]
    fn test_database_upsert() {
        let mut db = PluginDatabase::new();
        
        let mut entry = make_entry("Plugin", "VST3");
        entry.verified = false;
        db.upsert(entry);
        assert_eq!(db.count(), 1);
        
        // Update same plugin
        let mut entry = make_entry("Plugin", "VST3");
        entry.verified = true;
        db.upsert(entry);
        
        assert_eq!(db.count(), 1);
        assert!(db.plugins[0].verified);
    }
    
    #[test]
    fn test_database_count_by_format() {
        let mut db = PluginDatabase::new();
        db.upsert(make_entry("P1", "VST3"));
        db.upsert(make_entry("P2", "VST3"));
        db.upsert(make_entry("P3", "CLAP"));
        
        assert_eq!(db.count_by_format(PluginFormat::Vst3), 2);
        assert_eq!(db.count_by_format(PluginFormat::Clap), 1);
        assert_eq!(db.count_by_format(PluginFormat::AudioUnit), 0);
    }
    
    #[test]
    fn test_plugin_entry_format_conversion() {
        let entry = make_entry("Test", "VST3");
        assert_eq!(entry.format(), Some(PluginFormat::Vst3));
        
        let entry = make_entry("Test", "CLAP");
        assert_eq!(entry.format(), Some(PluginFormat::Clap));
        
        let entry = make_entry("Test", "Audio Unit");
        assert_eq!(entry.format(), Some(PluginFormat::AudioUnit));
    }
}
