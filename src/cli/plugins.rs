//! Plugin CLI - scan and manage plugins

use std::path::Path;
use crate::host::{PluginDatabase, PluginScanner, PluginFormat, ScannerConfig};

/// Scan for plugins in default and custom paths
pub fn scan_plugins(additional_paths: &[&str], formats: Option<&[PluginFormat]>) -> ScanResult {
    let mut config = ScannerConfig::default();
    
    // Add custom paths
    for path in additional_paths {
        config.search_paths.push(Path::new(path).to_path_buf());
    }
    
    // Filter formats if specified
    if let Some(fmts) = formats {
        config.formats = fmts.to_vec();
    }
    
    let scanner = PluginScanner::with_config(config);
    let plugins = scanner.scan();
    
    // Add to database
    let mut db = PluginDatabase::new();
    db.add_from_scan(&plugins);
    
    ScanResult {
        total: plugins.len(),
        vst3: plugins.iter().filter(|p| p.format == PluginFormat::Vst3).count(),
        au: plugins.iter().filter(|p| p.format == PluginFormat::AudioUnit).count(),
        clap: plugins.iter().filter(|p| p.format == PluginFormat::Clap).count(),
        database: db,
    }
}

/// Result of plugin scan
#[derive(Debug)]
pub struct ScanResult {
    pub total: usize,
    pub vst3: usize,
    pub au: usize,
    pub clap: usize,
    pub database: PluginDatabase,
}

/// List plugins from database
pub fn list_plugins(
    db: &PluginDatabase,
    format_filter: Option<PluginFormat>,
    name_filter: Option<&str>,
) -> Vec<PluginInfo> {
    let entries = if let Some(name) = name_filter {
        db.search_by_name(name)
    } else if let Some(format) = format_filter {
        db.filter_by_format(format)
    } else {
        db.all().iter().collect()
    };
    
    entries
        .into_iter()
        .map(|e| PluginInfo {
            name: e.name.clone(),
            path: e.path.clone(),
            format: e.format.clone(),
            vendor: e.vendor.clone(),
        })
        .collect()
}

/// Plugin info for display
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub name: String,
    pub path: String,
    pub format: String,
    pub vendor: Option<String>,
}

impl PluginInfo {
    pub fn format_line(&self) -> String {
        let vendor = self.vendor.as_deref().unwrap_or("Unknown");
        format!("[{}] {} by {} - {}", self.format, self.name, vendor, self.path)
    }
}

/// Get detailed info about a specific plugin
pub fn get_plugin_info(db: &PluginDatabase, path: &str) -> Option<DetailedPluginInfo> {
    let entry = db.get_by_path(path)?;
    
    Some(DetailedPluginInfo {
        name: entry.name.clone(),
        path: entry.path.clone(),
        format: entry.format.clone(),
        vendor: entry.vendor.clone(),
        category: entry.category.clone(),
        verified: entry.verified,
    })
}

/// Detailed plugin information
#[derive(Debug, Clone)]
pub struct DetailedPluginInfo {
    pub name: String,
    pub path: String,
    pub format: String,
    pub vendor: Option<String>,
    pub category: Option<String>,
    pub verified: bool,
}

/// Save database to default location
pub fn save_database(db: &PluginDatabase) -> Result<(), String> {
    let path = PluginDatabase::default_path();
    db.save(&path)
        .map_err(|e| format!("Failed to save database: {}", e))
}

/// Load database from default location
pub fn load_database() -> Result<PluginDatabase, String> {
    let path = PluginDatabase::default_path();
    if !path.exists() {
        return Ok(PluginDatabase::new());
    }
    PluginDatabase::load(&path)
        .map_err(|e| format!("Failed to load database: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;
    
    #[test]
    fn test_scan_empty_paths() {
        let temp = TempDir::new().unwrap();
        let result = scan_plugins(&[temp.path().to_str().unwrap()], None);
        assert_eq!(result.total, 0);
    }
    
    #[test]
    fn test_scan_with_plugins() {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join("Test.vst3")).unwrap();
        fs::create_dir(temp.path().join("Other.clap")).unwrap();
        
        let result = scan_plugins(&[temp.path().to_str().unwrap()], None);
        assert_eq!(result.total, 2);
        assert_eq!(result.vst3, 1);
        assert_eq!(result.clap, 1);
    }
    
    #[test]
    fn test_list_plugins() {
        let mut db = PluginDatabase::new();
        use crate::host::PluginEntry;
        
        db.upsert(PluginEntry {
            name: "Reverb".to_string(),
            path: "/test/reverb.vst3".to_string(),
            format: "VST3".to_string(),
            vendor: Some("TestVendor".to_string()),
            category: None,
            verified: false,
            last_scanned: 0,
        });
        
        let plugins = list_plugins(&db, None, None);
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].name, "Reverb");
    }
    
    #[test]
    fn test_plugin_info_format_line() {
        let info = PluginInfo {
            name: "TestPlugin".to_string(),
            path: "/path/test.vst3".to_string(),
            format: "VST3".to_string(),
            vendor: Some("MyVendor".to_string()),
        };
        
        let line = info.format_line();
        assert!(line.contains("VST3"));
        assert!(line.contains("TestPlugin"));
        assert!(line.contains("MyVendor"));
    }
}
