//! Plugin trait definition

use crate::buffer::AudioBuffer;
use crate::param::ParamInfo;
use crate::process::ProcessContext;
use crate::Result;
use super::config::PluginConfig;
use super::info::PluginInfo;

/// Core plugin trait
pub trait Plugin: Send + Sync {
    /// Get plugin information
    fn info(&self) -> PluginInfo;

    /// Initialize the plugin
    fn init(&mut self, config: &PluginConfig) -> Result<()>;

    /// Process audio
    fn process(&mut self, buffer: &mut AudioBuffer, ctx: &ProcessContext);

    /// Get parameter list
    fn parameters(&self) -> Vec<ParamInfo> {
        vec![]
    }

    /// Set a parameter by ID
    fn set_parameter(&mut self, id: u32, value: f32);

    /// Get a parameter by ID
    fn get_parameter(&self, id: u32) -> f32;

    /// Get plugin state for saving
    fn get_state(&self) -> Vec<u8> {
        vec![]
    }

    /// Restore plugin state
    fn set_state(&mut self, _data: &[u8]) -> Result<()> {
        Ok(())
    }

    /// Reset plugin state
    fn reset(&mut self) {}

    /// Get latency in samples
    fn latency(&self) -> u32 {
        0
    }

    /// Get tail time in samples (for reverb/delay)
    fn tail(&self) -> u32 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::PluginCategory;

    struct TestPlugin {
        gain: f32,
    }

    impl Plugin for TestPlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo {
                id: "test.plugin".to_string(),
                name: "Test Plugin".to_string(),
                vendor: "Test".to_string(),
                version: "1.0.0".to_string(),
                category: PluginCategory::Effect,
                inputs: 2,
                outputs: 2,
            }
        }

        fn init(&mut self, _config: &PluginConfig) -> Result<()> {
            Ok(())
        }

        fn process(&mut self, buffer: &mut AudioBuffer, _ctx: &ProcessContext) {
            for ch in 0..buffer.channels() {
                if let Some(channel) = buffer.channel_mut(ch) {
                    for sample in channel.iter_mut() {
                        *sample *= self.gain;
                    }
                }
            }
        }

        fn set_parameter(&mut self, id: u32, value: f32) {
            if id == 0 {
                self.gain = value;
            }
        }

        fn get_parameter(&self, id: u32) -> f32 {
            if id == 0 { self.gain } else { 0.0 }
        }
    }

    #[test]
    fn test_plugin_trait() {
        let mut plugin = TestPlugin { gain: 0.5 };
        assert_eq!(plugin.info().name, "Test Plugin");
        
        plugin.set_parameter(0, 0.8);
        assert_eq!(plugin.get_parameter(0), 0.8);
    }

    #[test]
    fn test_plugin_process() {
        let mut plugin = TestPlugin { gain: 0.5 };
        let mut buffer = AudioBuffer::from_interleaved(&[1.0, 1.0, 1.0, 1.0], 2);
        plugin.process(&mut buffer, &ProcessContext::default());
        
        assert!((buffer.channel(0).unwrap()[0] - 0.5).abs() < 0.001);
    }
}
