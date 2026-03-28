//! Compressor/limiter effect with sidechain and lookahead

use crate::buffer::AudioBuffer;
use crate::plugin::{Plugin, PluginCategory, PluginConfig, PluginInfo};
use crate::process::ProcessContext;
use crate::Result;

fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

fn linear_to_db(linear: f32) -> f32 {
    if linear > 1e-10 { 20.0 * linear.log10() } else { -200.0 }
}

/// Detection mode for compressor
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DetectionMode {
    Peak,
    Rms,
}

/// Compressor effect with sidechain and lookahead
#[derive(Clone)]
pub struct Compressor {
    threshold: f32,        // dB
    ratio: f32,
    attack: f32,           // seconds
    release: f32,          // seconds
    knee: f32,             // dB
    makeup: f32,           // dB
    envelope: f32,
    sample_rate: f32,
    // Detection mode
    detection: DetectionMode,
    rms_window: Vec<f32>,
    rms_index: usize,
    rms_sum: f32,
    // Sidechain
    sidechain_enabled: bool,
    sidechain_hpf: f32,    // High-pass filter freq for sidechain
    sidechain_state: f32,
    // Lookahead
    lookahead_ms: f32,
    lookahead_buffer_l: Vec<f32>,
    lookahead_buffer_r: Vec<f32>,
    lookahead_index: usize,
    lookahead_samples: usize,
}

impl Compressor {
    /// Create a new compressor
    pub fn new(sample_rate: u32) -> Self {
        let sample_rate_f = sample_rate as f32;
        let rms_samples = (sample_rate_f * 0.01) as usize; // 10ms RMS window
        
        Self {
            threshold: -20.0,
            ratio: 4.0,
            attack: 0.01,
            release: 0.1,
            knee: 6.0,
            makeup: 0.0,
            envelope: 0.0,
            sample_rate: sample_rate_f,
            detection: DetectionMode::Peak,
            rms_window: vec![0.0; rms_samples],
            rms_index: 0,
            rms_sum: 0.0,
            sidechain_enabled: false,
            sidechain_hpf: 100.0,
            sidechain_state: 0.0,
            lookahead_ms: 0.0,
            lookahead_buffer_l: Vec::new(),
            lookahead_buffer_r: Vec::new(),
            lookahead_index: 0,
            lookahead_samples: 0,
        }
    }

    /// Set threshold in dB
    pub fn set_threshold(&mut self, db: f32) {
        self.threshold = db.clamp(-60.0, 0.0);
    }

    /// Set ratio (1.0 = no compression, inf = limiting)
    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.clamp(1.0, 20.0);
    }

    /// Set attack time in seconds
    pub fn set_attack(&mut self, seconds: f32) {
        self.attack = seconds.clamp(0.0001, 1.0);
    }

    /// Set release time in seconds
    pub fn set_release(&mut self, seconds: f32) {
        self.release = seconds.clamp(0.01, 5.0);
    }

    /// Set soft knee width in dB
    pub fn set_knee(&mut self, db: f32) {
        self.knee = db.clamp(0.0, 24.0);
    }

    /// Set makeup gain in dB
    pub fn set_makeup(&mut self, db: f32) {
        self.makeup = db.clamp(0.0, 40.0);
    }

    /// Set detection mode (Peak or RMS)
    pub fn set_detection(&mut self, mode: DetectionMode) {
        self.detection = mode;
    }

    /// Enable/disable sidechain
    pub fn set_sidechain_enabled(&mut self, enabled: bool) {
        self.sidechain_enabled = enabled;
    }

    /// Set sidechain high-pass filter frequency
    pub fn set_sidechain_hpf(&mut self, freq: f32) {
        self.sidechain_hpf = freq.clamp(20.0, 500.0);
    }

    /// Set lookahead time in milliseconds (adds latency)
    pub fn set_lookahead(&mut self, ms: f32) {
        self.lookahead_ms = ms.clamp(0.0, 20.0);
        self.lookahead_samples = (self.lookahead_ms * self.sample_rate / 1000.0) as usize;
        
        if self.lookahead_samples > 0 {
            self.lookahead_buffer_l.resize(self.lookahead_samples, 0.0);
            self.lookahead_buffer_r.resize(self.lookahead_samples, 0.0);
            self.lookahead_index = 0;
        }
    }

    /// Get lookahead latency in samples
    pub fn lookahead_latency(&self) -> usize {
        self.lookahead_samples
    }

    /// Calculate auto makeup gain
    pub fn auto_makeup(&mut self) {
        let over = -self.threshold;
        if over > 0.0 {
            self.makeup = over * (1.0 - 1.0 / self.ratio);
        }
    }

    fn compute_gain(&self, input_db: f32) -> f32 {
        let over = input_db - self.threshold;

        if self.knee > 0.0 {
            let half_knee = self.knee / 2.0;
            if over < -half_knee {
                input_db
            } else if over > half_knee {
                self.threshold + over / self.ratio
            } else {
                let t = (over + half_knee) / self.knee;
                input_db - t * t * (over - over / self.ratio)
            }
        } else {
            if over <= 0.0 {
                input_db
            } else {
                self.threshold + over / self.ratio
            }
        }
    }

    fn apply_sidechain_hpf(&mut self, input: f32) -> f32 {
        let rc = 1.0 / (2.0 * std::f32::consts::PI * self.sidechain_hpf);
        let alpha = rc / (rc + 1.0 / self.sample_rate);
        let output = alpha * (self.sidechain_state + input - self.sidechain_state);
        self.sidechain_state = input;
        output
    }

    fn compute_rms(&mut self, input: f32) -> f32 {
        let squared = input * input;
        self.rms_sum -= self.rms_window[self.rms_index];
        self.rms_sum += squared;
        self.rms_window[self.rms_index] = squared;
        self.rms_index = (self.rms_index + 1) % self.rms_window.len();
        (self.rms_sum / self.rms_window.len() as f32).sqrt()
    }

    /// Process a mono sample
    pub fn process_sample(&mut self, input: f32) -> f32 {
        self.process_with_sidechain(input, input)
    }

    /// Process with external sidechain input
    pub fn process_with_sidechain(&mut self, input: f32, sidechain: f32) -> f32 {
        // Apply sidechain HPF if enabled
        let detect_signal = if self.sidechain_enabled {
            self.apply_sidechain_hpf(sidechain)
        } else {
            input
        };

        // Detection
        let level = match self.detection {
            DetectionMode::Peak => detect_signal.abs(),
            DetectionMode::Rms => self.compute_rms(detect_signal),
        };
        let input_db = linear_to_db(level);

        // Envelope follower
        let target = input_db;
        let coeff = if target > self.envelope {
            1.0 - (-1.0 / (self.attack * self.sample_rate)).exp()
        } else {
            1.0 - (-1.0 / (self.release * self.sample_rate)).exp()
        };
        self.envelope += coeff * (target - self.envelope);

        // Gain computation
        let output_db = self.compute_gain(self.envelope);
        let gain_db = output_db - self.envelope + self.makeup;
        let gain = db_to_linear(gain_db);

        // Apply lookahead delay if enabled
        if self.lookahead_samples > 0 {
            let delayed = self.lookahead_buffer_l[self.lookahead_index];
            self.lookahead_buffer_l[self.lookahead_index] = input;
            self.lookahead_index = (self.lookahead_index + 1) % self.lookahead_samples;
            delayed * gain
        } else {
            input * gain
        }
    }

    /// Process stereo (linked)
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.process_stereo_with_sidechain(left, right, (left + right) * 0.5)
    }

    /// Process stereo with external sidechain
    pub fn process_stereo_with_sidechain(&mut self, left: f32, right: f32, sidechain: f32) -> (f32, f32) {
        // Apply sidechain HPF if enabled
        let detect_signal = if self.sidechain_enabled {
            self.apply_sidechain_hpf(sidechain)
        } else {
            left.abs().max(right.abs())
        };

        // Detection
        let level = match self.detection {
            DetectionMode::Peak => detect_signal.abs(),
            DetectionMode::Rms => self.compute_rms(detect_signal),
        };
        let input_db = linear_to_db(level);

        // Envelope
        let target = input_db;
        let coeff = if target > self.envelope {
            1.0 - (-1.0 / (self.attack * self.sample_rate)).exp()
        } else {
            1.0 - (-1.0 / (self.release * self.sample_rate)).exp()
        };
        self.envelope += coeff * (target - self.envelope);

        let output_db = self.compute_gain(self.envelope);
        let gain_db = output_db - self.envelope + self.makeup;
        let gain = db_to_linear(gain_db);

        // Apply lookahead
        if self.lookahead_samples > 0 {
            let delayed_l = self.lookahead_buffer_l[self.lookahead_index];
            let delayed_r = self.lookahead_buffer_r[self.lookahead_index];
            self.lookahead_buffer_l[self.lookahead_index] = left;
            self.lookahead_buffer_r[self.lookahead_index] = right;
            self.lookahead_index = (self.lookahead_index + 1) % self.lookahead_samples;
            (delayed_l * gain, delayed_r * gain)
        } else {
            (left * gain, right * gain)
        }
    }

    /// Get current gain reduction in dB
    pub fn gain_reduction(&self) -> f32 {
        let output_db = self.compute_gain(self.envelope);
        (self.envelope - output_db).max(0.0)
    }
}

impl Plugin for Compressor {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: "pulse.compressor".to_string(),
            name: "Compressor".to_string(),
            vendor: "Pulse".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        }
    }

    fn init(&mut self, config: &PluginConfig) -> Result<()> {
        self.sample_rate = config.sample_rate;
        let rms_samples = (self.sample_rate * 0.01) as usize;
        self.rms_window.resize(rms_samples, 0.0);
        Ok(())
    }

    fn process(&mut self, buffer: &mut AudioBuffer, _ctx: &ProcessContext) {
        let frames = buffer.frames();
        let channels = buffer.channels();

        if channels >= 2 {
            for i in 0..frames {
                let left = buffer.channel(0).map(|c| c[i]).unwrap_or(0.0);
                let right = buffer.channel(1).map(|c| c[i]).unwrap_or(0.0);
                let (out_l, out_r) = self.process_stereo(left, right);
                if let Some(ch) = buffer.channel_mut(0) { ch[i] = out_l; }
                if let Some(ch) = buffer.channel_mut(1) { ch[i] = out_r; }
            }
        } else if channels == 1 {
            if let Some(ch) = buffer.channel_mut(0) {
                for sample in ch.iter_mut() {
                    *sample = self.process_sample(*sample);
                }
            }
        }
    }

    fn set_parameter(&mut self, id: u32, value: f32) {
        match id {
            0 => self.set_threshold(value * 60.0 - 60.0),
            1 => self.set_ratio(1.0 + value * 19.0),
            2 => self.set_attack(value * 0.999 + 0.001),
            3 => self.set_release(value * 4.99 + 0.01),
            4 => self.set_knee(value * 24.0),
            5 => self.set_makeup(value * 40.0),
            6 => self.set_detection(if value > 0.5 { DetectionMode::Rms } else { DetectionMode::Peak }),
            7 => self.set_lookahead(value * 20.0),
            8 => self.set_sidechain_enabled(value > 0.5),
            9 => self.set_sidechain_hpf(20.0 + value * 480.0),
            _ => {}
        }
    }

    fn get_parameter(&self, id: u32) -> f32 {
        match id {
            0 => (self.threshold + 60.0) / 60.0,
            1 => (self.ratio - 1.0) / 19.0,
            2 => (self.attack - 0.001) / 0.999,
            3 => (self.release - 0.01) / 4.99,
            4 => self.knee / 24.0,
            5 => self.makeup / 40.0,
            6 => if self.detection == DetectionMode::Rms { 1.0 } else { 0.0 },
            7 => self.lookahead_ms / 20.0,
            8 => if self.sidechain_enabled { 1.0 } else { 0.0 },
            9 => (self.sidechain_hpf - 20.0) / 480.0,
            _ => 0.0,
        }
    }

    fn reset(&mut self) {
        self.envelope = 0.0;
        self.rms_sum = 0.0;
        self.rms_window.fill(0.0);
        self.lookahead_buffer_l.fill(0.0);
        self.lookahead_buffer_r.fill(0.0);
        self.sidechain_state = 0.0;
    }

    fn latency(&self) -> u32 {
        self.lookahead_samples as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressor_new() {
        let comp = Compressor::new(44100);
        assert_eq!(comp.threshold, -20.0);
        assert_eq!(comp.ratio, 4.0);
    }

    #[test]
    fn test_compressor_below_threshold() {
        let mut comp = Compressor::new(44100);
        comp.set_threshold(-10.0);
        comp.set_makeup(0.0);
        let out = comp.process_sample(0.01);
        assert!((out - 0.01).abs() < 0.01);
    }

    #[test]
    fn test_compressor_above_threshold() {
        let mut comp = Compressor::new(44100);
        comp.set_threshold(-20.0);
        comp.set_ratio(4.0);
        comp.set_attack(0.0001);
        comp.set_makeup(0.0);

        let mut out = 0.0;
        for _ in 0..1000 {
            out = comp.process_sample(0.9);
        }
        assert!(out < 0.9);
    }

    #[test]
    fn test_compressor_stereo() {
        let mut comp = Compressor::new(44100);
        let (l, r) = comp.process_stereo(0.5, 0.5);
        assert!(l.is_finite());
        assert!(r.is_finite());
    }

    #[test]
    fn test_compressor_gain_reduction() {
        let mut comp = Compressor::new(44100);
        comp.set_threshold(-20.0);
        comp.set_attack(0.0001);

        for _ in 0..1000 {
            comp.process_sample(0.9);
        }

        let gr = comp.gain_reduction();
        assert!(gr > 0.0);
    }

    #[test]
    fn test_compressor_soft_knee() {
        let mut comp = Compressor::new(44100);
        comp.set_knee(12.0);
        let out = comp.process_sample(0.5);
        assert!(out.is_finite());
    }

    #[test]
    fn test_compressor_auto_makeup() {
        let mut comp = Compressor::new(44100);
        comp.set_threshold(-20.0);
        comp.set_ratio(4.0);
        comp.auto_makeup();
        assert!(comp.makeup > 0.0);
    }

    #[test]
    fn test_compressor_plugin_info() {
        let comp = Compressor::new(44100);
        assert_eq!(comp.info().name, "Compressor");
    }

    #[test]
    fn test_compressor_rms_detection() {
        let mut comp = Compressor::new(44100);
        comp.set_detection(DetectionMode::Rms);
        comp.set_threshold(-20.0);
        
        for _ in 0..1000 {
            comp.process_sample(0.5);
        }
        assert!(comp.gain_reduction() >= 0.0);
    }

    #[test]
    fn test_compressor_lookahead() {
        let mut comp = Compressor::new(44100);
        comp.set_lookahead(5.0); // 5ms
        
        assert!(comp.lookahead_latency() > 0);
        assert_eq!(comp.latency(), comp.lookahead_latency() as u32);
        
        // First samples should be delayed
        let out = comp.process_sample(1.0);
        assert_eq!(out, 0.0); // Delayed output starts at 0
    }

    #[test]
    fn test_compressor_sidechain() {
        let mut comp = Compressor::new(44100);
        comp.set_sidechain_enabled(true);
        comp.set_sidechain_hpf(200.0);
        
        // Process with sidechain
        let out = comp.process_with_sidechain(0.5, 0.8);
        assert!(out.is_finite());
    }
}
