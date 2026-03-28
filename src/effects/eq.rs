//! Parametric EQ with optional linear phase mode

use crate::buffer::AudioBuffer;
use crate::effects::filter::{BiquadFilter, FilterType};
use crate::plugin::{Plugin, PluginCategory, PluginConfig, PluginInfo};
use crate::process::ProcessContext;
use crate::Result;
use rustfft::{FftPlanner, num_complex::Complex};
use std::f32::consts::PI;

/// EQ band
#[derive(Clone)]
pub struct EqBand {
    filter_l: BiquadFilter,
    filter_r: BiquadFilter,
    pub freq: f32,
    pub gain: f32,
    pub q: f32,
    pub filter_type: FilterType,
    pub enabled: bool,
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
    linear_phase: bool,
    // Linear phase buffers
    fft_size: usize,
    input_buffer_l: Vec<f32>,
    input_buffer_r: Vec<f32>,
    output_buffer_l: Vec<f32>,
    output_buffer_r: Vec<f32>,
    buffer_pos: usize,
    overlap: usize,
}

impl ParametricEQ {
    /// Create a new parametric EQ
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as f32;
        let bands = vec![
            EqBand::new(80.0, FilterType::LowShelf, sr),
            EqBand::new(400.0, FilterType::Peak, sr),
            EqBand::new(2000.0, FilterType::Peak, sr),
            EqBand::new(8000.0, FilterType::HighShelf, sr),
        ];
        
        let fft_size = 4096;
        let overlap = fft_size / 2;
        
        Self {
            bands,
            sample_rate: sr,
            linear_phase: false,
            fft_size,
            input_buffer_l: vec![0.0; fft_size],
            input_buffer_r: vec![0.0; fft_size],
            output_buffer_l: vec![0.0; fft_size],
            output_buffer_r: vec![0.0; fft_size],
            buffer_pos: 0,
            overlap,
        }
    }

    /// Enable/disable linear phase mode
    pub fn set_linear_phase(&mut self, enabled: bool) {
        self.linear_phase = enabled;
        if enabled {
            // Reset buffers
            self.input_buffer_l.fill(0.0);
            self.input_buffer_r.fill(0.0);
            self.output_buffer_l.fill(0.0);
            self.output_buffer_r.fill(0.0);
            self.buffer_pos = 0;
        }
    }

    /// Check if linear phase is enabled
    pub fn is_linear_phase(&self) -> bool {
        self.linear_phase
    }

    /// Get latency (only non-zero in linear phase mode)
    pub fn latency(&self) -> usize {
        if self.linear_phase {
            self.fft_size / 2
        } else {
            0
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

    /// Compute magnitude response at a frequency
    fn compute_magnitude(&self, freq: f32) -> f32 {
        let mut magnitude = 1.0;
        
        for band in &self.bands {
            if !band.enabled {
                continue;
            }
            
            // Approximate biquad magnitude response
            let w = 2.0 * PI * freq / self.sample_rate;
            let w0 = 2.0 * PI * band.freq / self.sample_rate;
            let gain_linear = 10.0_f32.powf(band.gain / 20.0);
            
            match band.filter_type {
                FilterType::Peak => {
                    let bw = w0 / band.q;
                    let diff = (w - w0).abs();
                    if diff < bw {
                        let t = 1.0 - diff / bw;
                        magnitude *= 1.0 + (gain_linear - 1.0) * t * t;
                    }
                }
                FilterType::LowShelf => {
                    if freq < band.freq {
                        magnitude *= gain_linear;
                    } else {
                        let t = ((freq / band.freq).ln() / 2.0).tanh();
                        magnitude *= 1.0 + (gain_linear - 1.0) * (1.0 - t);
                    }
                }
                FilterType::HighShelf => {
                    if freq > band.freq {
                        magnitude *= gain_linear;
                    } else {
                        let t = ((band.freq / freq).ln() / 2.0).tanh();
                        magnitude *= 1.0 + (gain_linear - 1.0) * (1.0 - t);
                    }
                }
                FilterType::LowPass => {
                    if freq > band.freq {
                        let t = freq / band.freq;
                        magnitude *= 1.0 / (1.0 + t * t);
                    }
                }
                FilterType::HighPass => {
                    if freq < band.freq {
                        let t = band.freq / freq;
                        magnitude *= 1.0 / (1.0 + t * t);
                    }
                }
                FilterType::Notch => {
                    let bw = w0 / band.q;
                    let diff = (w - w0).abs();
                    if diff < bw {
                        magnitude *= diff / bw;
                    }
                }
                _ => {}
            }
        }
        
        magnitude
    }

    /// Process with linear phase (FFT-based)
    fn process_linear_phase(&mut self, buffer: &mut AudioBuffer) {
        let frames = buffer.frames();
        
        for i in 0..frames {
            // Add input to buffer
            let left = buffer.channel(0).map(|c| c[i]).unwrap_or(0.0);
            let right = buffer.channel(1).map(|c| c[i]).unwrap_or(0.0);
            
            self.input_buffer_l[self.buffer_pos] = left;
            self.input_buffer_r[self.buffer_pos] = right;
            
            // Output from overlap buffer
            let out_l = self.output_buffer_l[self.buffer_pos];
            let out_r = self.output_buffer_r[self.buffer_pos];
            
            if let Some(ch) = buffer.channel_mut(0) { ch[i] = out_l; }
            if let Some(ch) = buffer.channel_mut(1) { ch[i] = out_r; }
            
            self.buffer_pos += 1;
            
            // Process FFT block when buffer is full
            if self.buffer_pos >= self.overlap {
                self.process_fft_block();
                self.buffer_pos = 0;
            }
        }
    }

    fn process_fft_block(&mut self) {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(self.fft_size);
        let ifft = planner.plan_fft_inverse(self.fft_size);
        
        // Prepare complex buffers
        let mut spectrum_l: Vec<Complex<f32>> = self.input_buffer_l.iter()
            .map(|&x| Complex::new(x, 0.0))
            .collect();
        let mut spectrum_r: Vec<Complex<f32>> = self.input_buffer_r.iter()
            .map(|&x| Complex::new(x, 0.0))
            .collect();
        
        // Apply window (Hann)
        for i in 0..self.fft_size {
            let window = 0.5 * (1.0 - (2.0 * PI * i as f32 / self.fft_size as f32).cos());
            spectrum_l[i] = Complex::new(spectrum_l[i].re * window, 0.0);
            spectrum_r[i] = Complex::new(spectrum_r[i].re * window, 0.0);
        }
        
        // Forward FFT
        fft.process(&mut spectrum_l);
        fft.process(&mut spectrum_r);
        
        // Apply EQ magnitude (phase stays at zero for linear phase)
        let freq_resolution = self.sample_rate / self.fft_size as f32;
        for i in 0..=self.fft_size / 2 {
            let freq = i as f32 * freq_resolution;
            let mag = self.compute_magnitude(freq);
            
            spectrum_l[i] = Complex::new(spectrum_l[i].norm() * mag, 0.0);
            spectrum_r[i] = Complex::new(spectrum_r[i].norm() * mag, 0.0);
            
            // Mirror for negative frequencies
            if i > 0 && i < self.fft_size / 2 {
                spectrum_l[self.fft_size - i] = spectrum_l[i];
                spectrum_r[self.fft_size - i] = spectrum_r[i];
            }
        }
        
        // Inverse FFT
        ifft.process(&mut spectrum_l);
        ifft.process(&mut spectrum_r);
        
        // Normalize and overlap-add
        let norm = 1.0 / self.fft_size as f32;
        for i in 0..self.fft_size {
            let window = 0.5 * (1.0 - (2.0 * PI * i as f32 / self.fft_size as f32).cos());
            self.output_buffer_l[i] += spectrum_l[i].re * norm * window;
            self.output_buffer_r[i] += spectrum_r[i].re * norm * window;
        }
        
        // Shift output buffer
        let overlap = self.overlap;
        self.output_buffer_l.copy_within(overlap.., 0);
        self.output_buffer_r.copy_within(overlap.., 0);
        self.output_buffer_l[self.fft_size - overlap..].fill(0.0);
        self.output_buffer_r[self.fft_size - overlap..].fill(0.0);
        
        // Shift input buffer
        self.input_buffer_l.copy_within(overlap.., 0);
        self.input_buffer_r.copy_within(overlap.., 0);
    }

    /// Process stereo sample (minimum phase mode)
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
        if self.linear_phase {
            self.process_linear_phase(buffer);
        } else {
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
    }

    fn set_parameter(&mut self, id: u32, value: f32) {
        if id == 100 {
            self.set_linear_phase(value > 0.5);
            return;
        }
        
        let band = id as usize / 3;
        let param = id as usize % 3;
        
        match param {
            0 => self.set_band_freq(band, 20.0 * (1000.0_f32).powf(value)),
            1 => self.set_band_gain(band, value * 48.0 - 24.0),
            2 => self.set_band_q(band, 0.1 + value * 9.9),
            _ => {}
        }
    }

    fn get_parameter(&self, id: u32) -> f32 {
        if id == 100 {
            return if self.linear_phase { 1.0 } else { 0.0 };
        }
        
        let band = id as usize / 3;
        let param = id as usize % 3;
        
        if let Some(b) = self.bands.get(band) {
            match param {
                0 => (b.freq / 20.0).log10() / 3.0,
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
        self.input_buffer_l.fill(0.0);
        self.input_buffer_r.fill(0.0);
        self.output_buffer_l.fill(0.0);
        self.output_buffer_r.fill(0.0);
        self.buffer_pos = 0;
    }

    fn latency(&self) -> u32 {
        self.latency() as u32
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

    #[test]
    fn test_eq_linear_phase() {
        let mut eq = ParametricEQ::new(44100);
        assert!(!eq.is_linear_phase());
        
        eq.set_linear_phase(true);
        assert!(eq.is_linear_phase());
        assert!(eq.latency() > 0);
    }

    #[test]
    fn test_eq_linear_phase_process() {
        let mut eq = ParametricEQ::new(44100);
        eq.set_linear_phase(true);
        eq.set_band_gain(0, 6.0);
        
        let mut buffer = AudioBuffer::new(2, 256);
        // Fill with signal
        for i in 0..256 {
            let t = i as f32 / 44100.0;
            let sample = (t * 100.0 * 2.0 * PI).sin() * 0.5;
            buffer.channel_mut(0).unwrap()[i] = sample;
            buffer.channel_mut(1).unwrap()[i] = sample;
        }
        
        eq.process(&mut buffer, &ProcessContext::default());
        
        // Output should be finite
        for i in 0..256 {
            assert!(buffer.channel(0).unwrap()[i].is_finite());
        }
    }
}
