//! Preset format

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Plugin preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub plugin_id: String,
    pub name: String,
    pub parameters: HashMap<String, f32>,
    #[serde(default)]
    pub state: Vec<u8>,
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            name: "Default".to_string(),
            parameters: HashMap::new(),
            state: Vec::new(),
        }
    }
}
