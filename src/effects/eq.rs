//! Parametric EQ

use crate::buffer::AudioBuffer;
use crate::effects::filter::{BiquadFilter, FilterType};
use crate::plugin::{Plugin, PluginCategory, PluginConfig, PluginInfo};
use crate::process::ProcessContext;
use crate::Result;

/// EQ band
#[derive(Clone)]
pub struct EqBand {
    filter_l: BiquadFilter,
    filter_r: BiquadFilter,
    freq: f32,
    gain: f32,
    q: f32,
    filter_type: FilterType,
    enabled: bool,
    sample_rate: f32,
}

impl EqBand {
    fn new(freq: f32, filter_type: FilterType, sample_rate: f32) -> Self {
        let mut band = Self {
            filter_l: BiquadFilter::new(),
            filter_r: BiquadFilter::new(),
            freq,
            gain: 0.0,
            q: 0.707,
            filter_type,
            enabled: true,
            sample_rate,
        };
        band.update_coefficients();
        band
    }

    fn update_coefficients(&mut self) {
        self.filter_l.set_coefficients(self.filter_type, self.freq, self.q, self.gain, self.sample_rate);
        self.filter_r.set_coefficients(self.filter_type, self.freq, self.q, self.gain, self.sample_rate);
    }

    fn set_freq(&mut self, freq: f32) {
        self.freq = freq.clamp(20.0, 20000.0);
        self.update_coefficients();
    }

    fn set_gain(&mut self, gain: f32) {
        self.gain = gain.clamp(-24.0, 24.0);
        self.update_coefficients();
    }

    fn set_q(&mut self, q: f32) {
        self.q = q.clamp(0.1, 10.0);
        self.update_coefficients();
    }

    fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        if self.enabled {
            (self.filter_l.process(left), self.filter_r.process(right))
        } else {
            (left, right)
        }
    }

    fn reset(&mut self) {
        self.filter_l.reset();
        self.filter_r.reset();
    }
}

/// Parametric EQ with up to 8 bands
#[derive(Clone)]
pub struct ParametricEQ {
    bands: Vec<EqBand>,
    sample_rate: f32,
}

impl ParametricEQ {
    /// Create a new parametric EQ
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as f32;
        // Default 4-band EQ
        let bands = vec![
            EqBand::new(80.0, FilterType::LowShelf, sr),
            EqBand::new(400.0, FilterType::Peak, sr),
            EqBand::new(2000.0, FilterType::Peak, sr),
            EqBand::new(8000.0, FilterType::HighShelf, sr),
        ];
        
        Self {
            bands,
            sample_rate: sr,
        }
    }

    /// Add a band
    pub fn add_band(&mut self, freq: f32, filter_type: FilterType) {
        if self.bands.len() < 8 {
            self.bands.push(EqBand::new(freq, filter_type, self.sample_rate));
        }
    }

    /// Set band frequency
    pub fn set_band_freq(&mut self, band: usize, freq: f32) {
        if let Some(b) = self.bands.get_mut(band) {
            b.set_freq(freq);
        }
    }

    /// Set band gain
    pub fn set_band_gain(&mut self, band: usize, gain: f32) {
        if let Some(b) = self.bands.get_mut(band) {
            b.set_gain(gain);
        }
    }

    /// Set band Q
    pub fn set_band_q(&mut self, band: usize, q: f32) {
        if let Some(b) = self.bands.get_mut(band) {
            b.set_q(q);
        }
    }

    /// Enable/disable band
    pub fn set_band_enabled(&mut self, band: usize, enabled: bool) {
        if let Some(b) = self.bands.get_mut(band) {
            b.enabled = enabled;
        }
    }

    /// Get number of bands
    pub fn num_bands(&self) -> usize {
        self.bands.len()
    }

    /// Process stereo sample
    pub fn process_sample(&mut self, mut left: f32, mut right: f32) -> (f32, f32) {
        for band in &mut self.bands {
            let (l, r) = band.process(left, right);
            left = l;
            right = r;
        }
        (left, right)
    }
}

impl Plugin for ParametricEQ {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: "pulse.eq".to_string(),
            name: "Parametric EQ".to_string(),
            vendor: "Pulse".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        }
    }

    fn init(&mut self, config: &PluginConfig) -> Result<()> {
        if (config.sample_rate - self.sample_rate).abs() > 1.0 {
            *self = ParametricEQ::new(config.sample_rate as u32);
        }
        Ok(())
    }

    fn process(&mut self, buffer: &mut AudioBuffer, _ctx: &ProcessContext) {
        let frames = buffer.frames();
        if buffer.channels() < 2 {
            return;
        }

        for i in 0..frames {
            let left = buffer.channel(0).map(|c| c[i]).unwrap_or(0.0);
            let right = buffer.channel(1).map(|c| c[i]).unwrap_or(0.0);
            let (out_l, out_r) = self.process_sample(left, right);
            if let Some(ch) = buffer.channel_mut(0) { ch[i] = out_l; }
            if let Some(ch) = buffer.channel_mut(1) { ch[i] = out_r; }
        }
    }

    fn set_parameter(&mut self, id: u32, value: f32) {
        // Parameters organized as: band0_freq, band0_gain, band0_q, band1_freq, ...
        let band = id as usize / 3;
        let param = id as usize % 3;
        
        match param {
            0 => self.set_band_freq(band, 20.0 * (1000.0_f32).powf(value)), // Log scale 20-20000
            1 => self.set_band_gain(band, value * 48.0 - 24.0),              // -24 to +24 dB
            2 => self.set_band_q(band, 0.1 + value * 9.9),                   // 0.1 to 10
            _ => {}
        }
    }

    fn get_parameter(&self, id: u32) -> f32 {
        let band = id as usize / 3;
        let param = id as usize % 3;
        
        if let Some(b) = self.bands.get(band) {
            match param {
                0 => (b.freq / 20.0).log10() / 3.0, // Log scale
                1 => (b.gain + 24.0) / 48.0,
                2 => (b.q - 0.1) / 9.9,
                _ => 0.0,
            }
        } else {
            0.0
        }
    }

    fn reset(&mut self) {
        for band in &mut self.bands {
            band.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eq_new() {
        let eq = ParametricEQ::new(44100);
        assert_eq!(eq.num_bands(), 4);
    }

    #[test]
    fn test_eq_add_band() {
        let mut eq = ParametricEQ::new(44100);
        eq.add_band(1000.0, FilterType::Peak);
        assert_eq!(eq.num_bands(), 5);
    }

    #[test]
    fn test_eq_process() {
        let mut eq = ParametricEQ::new(44100);
        let (l, r) = eq.process_sample(0.5, 0.5);
        assert!(l.is_finite());
        assert!(r.is_finite());
    }

    #[test]
    fn test_eq_band_settings() {
        let mut eq = ParametricEQ::new(44100);
        eq.set_band_freq(0, 100.0);
        eq.set_band_gain(0, 6.0);
        eq.set_band_q(0, 2.0);
        
        // Should still process
        let (l, _) = eq.process_sample(0.5, 0.5);
        assert!(l.is_finite());
    }

    #[test]
    fn test_eq_disable_band() {
        let mut eq = ParametricEQ::new(44100);
        eq.set_band_enabled(0, false);
        let (l, _) = eq.process_sample(0.5, 0.5);
        assert!(l.is_finite());
    }

    #[test]
    fn test_eq_plugin_info() {
        let eq = ParametricEQ::new(44100);
        assert_eq!(eq.info().name, "Parametric EQ");
    }

    #[test]
    fn test_eq_max_bands() {
        let mut eq = ParametricEQ::new(44100);
        for _ in 0..10 {
            eq.add_band(1000.0, FilterType::Peak);
        }
        assert!(eq.num_bands() <= 8);
    }
}
