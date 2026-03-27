//! Plugin instance - unified wrapper for loaded plugins

use std::path::PathBuf;
use crate::buffer::AudioBuffer;
use crate::process::ProcessContext;
use crate::param::{ParamInfo, ParamValue};
use crate::host::scanner::PluginFormat;

/// Plugin instance state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceState {
    /// Instance created but not initialized
    Created,
    /// Instance initialized and ready
    Ready,
    /// Instance is processing audio
    Processing,
    /// Instance has been deactivated
    Inactive,
    /// Instance encountered an error
    Error,
}

/// Plugin instance identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InstanceId(pub u32);

impl InstanceId {
    pub fn new(id: u32) -> Self {
        Self(id)
    }
}

/// Plugin instance capabilities
#[derive(Debug, Clone, Default)]
pub struct InstanceCapabilities {
    /// Supports stereo processing
    pub stereo: bool,
    /// Supports mono processing
    pub mono: bool,
    /// Supports side-chain input
    pub sidechain: bool,
    /// Number of audio inputs
    pub audio_inputs: u32,
    /// Number of audio outputs
    pub audio_outputs: u32,
    /// Number of parameters
    pub param_count: u32,
    /// Supports preset loading
    pub presets: bool,
    /// Supports parameter automation
    pub automation: bool,
}

/// Plugin instance - wraps a loaded plugin
pub struct PluginInstance {
    /// Unique instance ID
    pub id: InstanceId,
    /// Plugin name
    pub name: String,
    /// Plugin vendor
    pub vendor: String,
    /// Plugin format
    pub format: PluginFormat,
    /// Path to plugin bundle
    pub path: PathBuf,
    /// Current state
    pub state: InstanceState,
    /// Sample rate
    pub sample_rate: f32,
    /// Maximum block size
    pub max_block_size: u32,
    /// Instance capabilities
    pub capabilities: InstanceCapabilities,
    /// Parameter values (id -> value)
    params: Vec<ParamValue>,
    /// Parameter info
    param_info: Vec<ParamInfo>,
    /// Bypass state
    bypass: bool,
}

impl PluginInstance {
    /// Create a new plugin instance
    pub fn new(
        id: InstanceId,
        name: impl Into<String>,
        vendor: impl Into<String>,
        format: PluginFormat,
        path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            vendor: vendor.into(),
            format,
            path: path.into(),
            state: InstanceState::Created,
            sample_rate: 44100.0,
            max_block_size: 512,
            capabilities: InstanceCapabilities::default(),
            params: Vec::new(),
            param_info: Vec::new(),
            bypass: false,
        }
    }
    
    /// Initialize the instance with sample rate and block size
    pub fn init(&mut self, sample_rate: f32, max_block_size: u32) -> Result<(), String> {
        if self.state != InstanceState::Created && self.state != InstanceState::Inactive {
            return Err("Instance already initialized".into());
        }
        
        self.sample_rate = sample_rate;
        self.max_block_size = max_block_size;
        self.state = InstanceState::Ready;
        Ok(())
    }
    
    /// Activate for processing
    pub fn activate(&mut self) -> Result<(), String> {
        if self.state != InstanceState::Ready && self.state != InstanceState::Inactive {
            return Err("Instance not ready".into());
        }
        self.state = InstanceState::Processing;
        Ok(())
    }
    
    /// Deactivate (stop processing)
    pub fn deactivate(&mut self) {
        if self.state == InstanceState::Processing {
            self.state = InstanceState::Inactive;
        }
    }
    
    /// Check if instance is ready to process
    pub fn is_processing(&self) -> bool {
        self.state == InstanceState::Processing
    }
    
    /// Process audio through the plugin
    pub fn process(&mut self, _buffer: &mut AudioBuffer, _context: &ProcessContext) -> Result<(), String> {
        if !self.is_processing() {
            return Err("Instance not processing".into());
        }
        
        if self.bypass {
            return Ok(());
        }
        
        // Actual processing would happen here via FFI
        // For now, this is a no-op placeholder
        Ok(())
    }
    
    /// Set bypass state
    pub fn set_bypass(&mut self, bypass: bool) {
        self.bypass = bypass;
    }
    
    /// Get bypass state
    pub fn is_bypassed(&self) -> bool {
        self.bypass
    }
    
    /// Get parameter count
    pub fn param_count(&self) -> usize {
        self.param_info.len()
    }
    
    /// Get parameter info
    pub fn get_param_info(&self, index: usize) -> Option<&ParamInfo> {
        self.param_info.get(index)
    }
    
    /// Get parameter value
    pub fn get_param(&self, index: usize) -> Option<&ParamValue> {
        self.params.get(index)
    }
    
    /// Set parameter value
    pub fn set_param(&mut self, index: usize, value: ParamValue) -> Result<(), String> {
        if index >= self.params.len() {
            return Err(format!("Parameter index {} out of range", index));
        }
        self.params[index] = value;
        Ok(())
    }
    
    /// Add a parameter (during initialization)
    pub fn add_param(&mut self, info: ParamInfo, default_value: ParamValue) {
        self.param_info.push(info);
        self.params.push(default_value);
    }
    
    /// Reset all parameters to defaults
    pub fn reset_params(&mut self) {
        for (idx, info) in self.param_info.iter().enumerate() {
            if idx < self.params.len() {
                self.params[idx] = info.param_type.default_value();
            }
        }
    }
    
    /// Get state data for saving
    pub fn get_state(&self) -> Vec<u8> {
        // Would serialize plugin state
        // For now, return param values as JSON
        serde_json::to_vec(&self.params).unwrap_or_default()
    }
    
    /// Restore state
    pub fn set_state(&mut self, data: &[u8]) -> Result<(), String> {
        let values: Vec<ParamValue> = serde_json::from_slice(data)
            .map_err(|e| format!("Failed to parse state: {}", e))?;
        
        if values.len() != self.params.len() {
            return Err("State parameter count mismatch".into());
        }
        
        self.params = values;
        Ok(())
    }
}

/// Plugin instance manager
#[derive(Default)]
pub struct InstanceManager {
    instances: Vec<PluginInstance>,
    next_id: u32,
}

impl InstanceManager {
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            next_id: 1,
        }
    }
    
    /// Create a new instance (returns ID)
    pub fn create(
        &mut self,
        name: impl Into<String>,
        vendor: impl Into<String>,
        format: PluginFormat,
        path: impl Into<PathBuf>,
    ) -> InstanceId {
        let id = InstanceId::new(self.next_id);
        self.next_id += 1;
        
        let instance = PluginInstance::new(id, name, vendor, format, path);
        self.instances.push(instance);
        id
    }
    
    /// Get instance by ID
    pub fn get(&self, id: InstanceId) -> Option<&PluginInstance> {
        self.instances.iter().find(|i| i.id == id)
    }
    
    /// Get mutable instance by ID
    pub fn get_mut(&mut self, id: InstanceId) -> Option<&mut PluginInstance> {
        self.instances.iter_mut().find(|i| i.id == id)
    }
    
    /// Remove instance
    pub fn remove(&mut self, id: InstanceId) -> Option<PluginInstance> {
        let pos = self.instances.iter().position(|i| i.id == id)?;
        Some(self.instances.remove(pos))
    }
    
    /// Get all instances
    pub fn all(&self) -> &[PluginInstance] {
        &self.instances
    }
    
    /// Count instances
    pub fn count(&self) -> usize {
        self.instances.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_instance_new() {
        let instance = PluginInstance::new(
            InstanceId::new(1),
            "TestPlugin",
            "TestVendor",
            PluginFormat::Clap,
            "/test/plugin.clap",
        );
        
        assert_eq!(instance.name, "TestPlugin");
        assert_eq!(instance.vendor, "TestVendor");
        assert_eq!(instance.state, InstanceState::Created);
    }
    
    #[test]
    fn test_instance_init() {
        let mut instance = PluginInstance::new(
            InstanceId::new(1),
            "Test",
            "Vendor",
            PluginFormat::Vst3,
            "/test.vst3",
        );
        
        assert!(instance.init(48000.0, 1024).is_ok());
        assert_eq!(instance.state, InstanceState::Ready);
        assert_eq!(instance.sample_rate, 48000.0);
        assert_eq!(instance.max_block_size, 1024);
    }
    
    #[test]
    fn test_instance_activate() {
        let mut instance = PluginInstance::new(
            InstanceId::new(1),
            "Test",
            "Vendor",
            PluginFormat::Clap,
            "/test.clap",
        );
        
        // Can't activate before init
        assert!(instance.activate().is_err());
        
        instance.init(44100.0, 512).unwrap();
        assert!(instance.activate().is_ok());
        assert!(instance.is_processing());
    }
    
    #[test]
    fn test_instance_bypass() {
        let mut instance = PluginInstance::new(
            InstanceId::new(1),
            "Test",
            "Vendor",
            PluginFormat::Clap,
            "/test.clap",
        );
        
        assert!(!instance.is_bypassed());
        instance.set_bypass(true);
        assert!(instance.is_bypassed());
    }
    
    #[test]
    fn test_instance_manager_create() {
        let mut manager = InstanceManager::new();
        
        let id1 = manager.create("Plugin1", "Vendor", PluginFormat::Vst3, "/p1.vst3");
        let id2 = manager.create("Plugin2", "Vendor", PluginFormat::Clap, "/p2.clap");
        
        assert_eq!(manager.count(), 2);
        assert_ne!(id1, id2);
    }
    
    #[test]
    fn test_instance_manager_get() {
        let mut manager = InstanceManager::new();
        let id = manager.create("Plugin", "Vendor", PluginFormat::Clap, "/test.clap");
        
        let instance = manager.get(id).unwrap();
        assert_eq!(instance.name, "Plugin");
    }
    
    #[test]
    fn test_instance_manager_remove() {
        let mut manager = InstanceManager::new();
        let id = manager.create("Plugin", "Vendor", PluginFormat::Clap, "/test.clap");
        
        assert_eq!(manager.count(), 1);
        let removed = manager.remove(id);
        assert!(removed.is_some());
        assert_eq!(manager.count(), 0);
    }
    
    #[test]
    fn test_instance_params() {
        let mut instance = PluginInstance::new(
            InstanceId::new(1),
            "Test",
            "Vendor",
            PluginFormat::Clap,
            "/test.clap",
        );
        
        let info = ParamInfo::float(0, "Volume", 0.0, 1.0, 0.5);
        
        instance.add_param(info, ParamValue::Float(0.5));
        assert_eq!(instance.param_count(), 1);
        
        instance.set_param(0, ParamValue::Float(0.8)).unwrap();
        assert_eq!(instance.get_param(0), Some(&ParamValue::Float(0.8)));
    }
}
