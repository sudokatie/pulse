//! Plugin configuration

/// Plugin configuration
#[derive(Debug, Clone)]
pub struct PluginConfig {
    /// Sample rate in Hz
    pub sample_rate: f32,
    /// Maximum block size
    pub max_block_size: usize,
    /// Number of input channels
    pub inputs: usize,
    /// Number of output channels
    pub outputs: usize,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100.0,
            max_block_size: 512,
            inputs: 2,
            outputs: 2,
        }
    }
}

impl PluginConfig {
    /// Create with specific sample rate
    pub fn with_sample_rate(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let cfg = PluginConfig::default();
        assert_eq!(cfg.sample_rate, 44100.0);
    }

    #[test]
    fn test_config_with_sample_rate() {
        let cfg = PluginConfig::with_sample_rate(48000.0);
        assert_eq!(cfg.sample_rate, 48000.0);
    }
}
