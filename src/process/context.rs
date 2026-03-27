//! Processing context

use super::transport::TransportState;

/// Processing context passed to plugins
#[derive(Debug, Clone)]
pub struct ProcessContext {
    /// Sample rate in Hz
    pub sample_rate: f32,
    /// Current block size
    pub block_size: usize,
    /// Tempo in BPM
    pub tempo: f64,
    /// Time signature (numerator, denominator)
    pub time_sig: (u32, u32),
    /// Transport state
    pub transport: TransportState,
}

impl Default for ProcessContext {
    fn default() -> Self {
        Self {
            sample_rate: 44100.0,
            block_size: 256,
            tempo: 120.0,
            time_sig: (4, 4),
            transport: TransportState::default(),
        }
    }
}

impl ProcessContext {
    /// Create with sample rate
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            ..Default::default()
        }
    }

    /// Get samples per beat
    pub fn samples_per_beat(&self) -> f64 {
        self.sample_rate as f64 * 60.0 / self.tempo
    }

    /// Get samples per bar
    pub fn samples_per_bar(&self) -> f64 {
        self.samples_per_beat() * self.time_sig.0 as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_default() {
        let ctx = ProcessContext::default();
        assert_eq!(ctx.sample_rate, 44100.0);
        assert_eq!(ctx.tempo, 120.0);
    }

    #[test]
    fn test_samples_per_beat() {
        let ctx = ProcessContext::default();
        // At 120 BPM and 44100 Hz: 44100 * 60 / 120 = 22050
        assert!((ctx.samples_per_beat() - 22050.0).abs() < 1.0);
    }
}
