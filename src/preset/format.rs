//! Preset format and management

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Plugin preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    /// Plugin ID this preset belongs to
    pub plugin_id: String,
    /// Preset name
    pub name: String,
    /// Author/creator
    #[serde(default)]
    pub author: String,
    /// Description
    #[serde(default)]
    pub description: String,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Parameter values (name -> value)
    pub parameters: HashMap<String, f32>,
    /// Raw plugin state data
    #[serde(default)]
    pub state: Vec<u8>,
    /// Version
    #[serde(default = "default_version")]
    pub version: u32,
}

fn default_version() -> u32 {
    1
}

impl Preset {
    /// Create a new preset
    pub fn new(plugin_id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            name: name.into(),
            author: String::new(),
            description: String::new(),
            tags: Vec::new(),
            parameters: HashMap::new(),
            state: Vec::new(),
            version: 1,
        }
    }
    
    /// Set a parameter value
    pub fn set_param(&mut self, name: impl Into<String>, value: f32) {
        self.parameters.insert(name.into(), value);
    }
    
    /// Get a parameter value
    pub fn get_param(&self, name: &str) -> Option<f32> {
        self.parameters.get(name).copied()
    }
    
    /// Load preset from file
    pub fn load(path: &Path) -> io::Result<Self> {
        let contents = fs::read_to_string(path)?;
        serde_json::from_str(&contents)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
    
    /// Save preset to file
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(path, contents)
    }
    
    /// Add a tag
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let tag = tag.into();
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }
    
    /// Check if preset has a tag
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
    }
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            name: "Default".to_string(),
            author: String::new(),
            description: String::new(),
            tags: Vec::new(),
            parameters: HashMap::new(),
            state: Vec::new(),
            version: 1,
        }
    }
}

/// Preset bank - collection of presets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetBank {
    /// Bank name
    pub name: String,
    /// Plugin ID this bank belongs to
    pub plugin_id: String,
    /// Presets in the bank
    pub presets: Vec<Preset>,
    /// Bank version
    #[serde(default = "default_version")]
    pub version: u32,
}

impl PresetBank {
    /// Create a new bank
    pub fn new(name: impl Into<String>, plugin_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            plugin_id: plugin_id.into(),
            presets: Vec::new(),
            version: 1,
        }
    }
    
    /// Add a preset to the bank
    pub fn add(&mut self, preset: Preset) {
        self.presets.push(preset);
    }
    
    /// Remove a preset by name
    pub fn remove(&mut self, name: &str) -> Option<Preset> {
        let pos = self.presets.iter().position(|p| p.name == name)?;
        Some(self.presets.remove(pos))
    }
    
    /// Get a preset by name
    pub fn get(&self, name: &str) -> Option<&Preset> {
        self.presets.iter().find(|p| p.name == name)
    }
    
    /// Get mutable preset by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Preset> {
        self.presets.iter_mut().find(|p| p.name == name)
    }
    
    /// Search presets by tag
    pub fn search_by_tag(&self, tag: &str) -> Vec<&Preset> {
        self.presets.iter().filter(|p| p.has_tag(tag)).collect()
    }
    
    /// Search presets by name (case-insensitive)
    pub fn search_by_name(&self, query: &str) -> Vec<&Preset> {
        let query_lower = query.to_lowercase();
        self.presets
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&query_lower))
            .collect()
    }
    
    /// List all preset names
    pub fn names(&self) -> Vec<&str> {
        self.presets.iter().map(|p| p.name.as_str()).collect()
    }
    
    /// Count presets
    pub fn count(&self) -> usize {
        self.presets.len()
    }
    
    /// Load bank from file
    pub fn load(path: &Path) -> io::Result<Self> {
        let contents = fs::read_to_string(path)?;
        serde_json::from_str(&contents)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
    
    /// Save bank to file
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        fs::write(path, contents)
    }
}

/// Preset manager - handles preset storage locations
pub struct PresetManager {
    /// Base directory for presets
    base_dir: PathBuf,
}

impl PresetManager {
    /// Create a new preset manager
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }
    
    /// Create with default directory
    pub fn with_default_dir() -> Self {
        let base = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("pulse")
            .join("presets");
        Self::new(base)
    }
    
    /// Get preset directory for a plugin
    pub fn plugin_dir(&self, plugin_id: &str) -> PathBuf {
        self.base_dir.join(plugin_id.replace(['/', '\\', ':'], "_"))
    }
    
    /// List all banks for a plugin
    pub fn list_banks(&self, plugin_id: &str) -> io::Result<Vec<String>> {
        let dir = self.plugin_dir(plugin_id);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        
        let mut banks = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    banks.push(stem.to_string());
                }
            }
        }
        Ok(banks)
    }
    
    /// Load a bank
    pub fn load_bank(&self, plugin_id: &str, bank_name: &str) -> io::Result<PresetBank> {
        let path = self.plugin_dir(plugin_id).join(format!("{}.json", bank_name));
        PresetBank::load(&path)
    }
    
    /// Save a bank
    pub fn save_bank(&self, bank: &PresetBank) -> io::Result<()> {
        let path = self.plugin_dir(&bank.plugin_id).join(format!("{}.json", bank.name));
        bank.save(&path)
    }
    
    /// Delete a bank
    pub fn delete_bank(&self, plugin_id: &str, bank_name: &str) -> io::Result<()> {
        let path = self.plugin_dir(plugin_id).join(format!("{}.json", bank_name));
        fs::remove_file(path)
    }
}

impl Default for PresetManager {
    fn default() -> Self {
        Self::with_default_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_preset_new() {
        let preset = Preset::new("com.test.synth", "Warm Pad");
        assert_eq!(preset.plugin_id, "com.test.synth");
        assert_eq!(preset.name, "Warm Pad");
        assert!(preset.parameters.is_empty());
    }
    
    #[test]
    fn test_preset_params() {
        let mut preset = Preset::new("test", "Test");
        preset.set_param("volume", 0.8);
        preset.set_param("pan", 0.0);
        
        assert_eq!(preset.get_param("volume"), Some(0.8));
        assert_eq!(preset.get_param("pan"), Some(0.0));
        assert_eq!(preset.get_param("missing"), None);
    }
    
    #[test]
    fn test_preset_tags() {
        let mut preset = Preset::new("test", "Test");
        preset.add_tag("pad");
        preset.add_tag("ambient");
        preset.add_tag("pad"); // duplicate
        
        assert_eq!(preset.tags.len(), 2);
        assert!(preset.has_tag("pad"));
        assert!(preset.has_tag("PAD")); // case insensitive
        assert!(!preset.has_tag("bass"));
    }
    
    #[test]
    fn test_preset_save_load() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("test.json");
        
        let mut preset = Preset::new("test", "My Preset");
        preset.set_param("cutoff", 0.5);
        preset.save(&path).unwrap();
        
        let loaded = Preset::load(&path).unwrap();
        assert_eq!(loaded.name, "My Preset");
        assert_eq!(loaded.get_param("cutoff"), Some(0.5));
    }
    
    #[test]
    fn test_bank_new() {
        let bank = PresetBank::new("Factory", "com.test.synth");
        assert_eq!(bank.name, "Factory");
        assert_eq!(bank.plugin_id, "com.test.synth");
        assert_eq!(bank.count(), 0);
    }
    
    #[test]
    fn test_bank_add_remove() {
        let mut bank = PresetBank::new("Test", "test");
        bank.add(Preset::new("test", "Preset 1"));
        bank.add(Preset::new("test", "Preset 2"));
        
        assert_eq!(bank.count(), 2);
        
        let removed = bank.remove("Preset 1");
        assert!(removed.is_some());
        assert_eq!(bank.count(), 1);
    }
    
    #[test]
    fn test_bank_search() {
        let mut bank = PresetBank::new("Test", "test");
        
        let mut p1 = Preset::new("test", "Warm Pad");
        p1.add_tag("pad");
        bank.add(p1);
        
        let mut p2 = Preset::new("test", "Deep Bass");
        p2.add_tag("bass");
        bank.add(p2);
        
        let pads = bank.search_by_tag("pad");
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].name, "Warm Pad");
        
        let search = bank.search_by_name("deep");
        assert_eq!(search.len(), 1);
    }
    
    #[test]
    fn test_bank_save_load() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("bank.json");
        
        let mut bank = PresetBank::new("Factory", "test");
        bank.add(Preset::new("test", "Init"));
        bank.save(&path).unwrap();
        
        let loaded = PresetBank::load(&path).unwrap();
        assert_eq!(loaded.name, "Factory");
        assert_eq!(loaded.count(), 1);
    }
    
    #[test]
    fn test_preset_manager() {
        let temp = TempDir::new().unwrap();
        let manager = PresetManager::new(temp.path());
        
        let mut bank = PresetBank::new("Factory", "com.test.synth");
        bank.add(Preset::new("com.test.synth", "Init"));
        manager.save_bank(&bank).unwrap();
        
        let banks = manager.list_banks("com.test.synth").unwrap();
        assert_eq!(banks.len(), 1);
        assert_eq!(banks[0], "Factory");
        
        let loaded = manager.load_bank("com.test.synth", "Factory").unwrap();
        assert_eq!(loaded.count(), 1);
    }
}
