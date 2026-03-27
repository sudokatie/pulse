//! Plugin information

use serde::{Deserialize, Serialize};

/// Plugin category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginCategory {
    Effect,
    Instrument,
    Analyzer,
    Generator,
    Other,
}

/// Plugin information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Unique plugin ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Vendor/manufacturer
    pub vendor: String,
    /// Version string
    pub version: String,
    /// Plugin category
    pub category: PluginCategory,
    /// Number of audio inputs
    pub inputs: usize,
    /// Number of audio outputs
    pub outputs: usize,
}

impl Default for PluginInfo {
    fn default() -> Self {
        Self {
            id: "unknown".to_string(),
            name: "Unknown Plugin".to_string(),
            vendor: "Unknown".to_string(),
            version: "0.0.0".to_string(),
            category: PluginCategory::Other,
            inputs: 2,
            outputs: 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_info_default() {
        let info = PluginInfo::default();
        assert_eq!(info.id, "unknown");
    }

    #[test]
    fn test_plugin_category() {
        let cat = PluginCategory::Effect;
        assert_eq!(cat, PluginCategory::Effect);
    }
}
