//! Stereo delay effect with tempo sync, modulation, and saturation

use crate::buffer::AudioBuffer;
use crate::plugin::{Plugin, PluginCategory, PluginConfig, PluginInfo};
use crate::process::ProcessContext;
use crate::Result;
use std::f32::consts::PI;

/// Delay line buffer with interpolated reads
#[derive(Clone)]
struct DelayLine {
    buffer: Vec<f32>,
    write_pos: usize,
}

impl DelayLine {
    fn new(max_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; max_samples.max(1)],
            write_pos: 0,
        }
    }

    fn write(&mut self, sample: f32) {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
    }

    fn read(&self, delay: usize) -> f32 {
        let delay = delay.min(self.buffer.len() - 1);
        let read_pos = (self.write_pos + self.buffer.len() - 1 - delay) % self.buffer.len();
        self.buffer[read_pos]
    }

    fn read_interp(&self, delay: f32) -> f32 {
        let delay = delay.clamp(0.0, (self.buffer.len() - 1) as f32);
        let d0 = delay.floor() as usize;
        let d1 = (d0 + 1).min(self.buffer.len() - 1);
        let frac = delay - d0 as f32;
        self.read(d0) * (1.0 - frac) + self.read(d1) * frac
    }

    fn clear(&mut self) {
        self.buffer.fill(0.0);
    }
}

/// LFO for delay modulation
#[derive(Clone)]
struct Lfo {
    phase: f32,
    rate: f32,        // Hz
    sample_rate: f32,
}

impl Lfo {
    fn new(sample_rate: f32) -> Self {
        Self {
            phase: 0.0,
            rate: 0.5,
            sample_rate,
        }
    }

    fn set_rate(&mut self, rate: f32) {
        self.rate = rate.clamp(0.01, 10.0);
    }

    fn next(&mut self) -> f32 {
        let out = (self.phase * 2.0 * PI).sin();
        self.phase += self.rate / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        out
    }

    fn reset(&mut self) {
        self.phase = 0.0;
    }
}

/// Soft saturation for feedback
fn saturate(x: f32, drive: f32) -> f32 {
    if drive <= 0.0 {
        return x;
    }
    let driven = x * (1.0 + drive * 2.0);
    driven.tanh() / (1.0 + drive * 0.5).tanh()
}

/// Stereo delay effect with modulation and saturation
#[derive(Clone)]
pub struct Delay {
    delay_l: DelayLine,
    delay_r: DelayLine,
    time_l: f32,           // seconds
    time_r: f32,           // seconds
    feedback: f32,
    cross_feedback: f32,   // For ping-pong
    mix: f32,
    sample_rate: f32,
    // Modulation
    lfo_l: Lfo,
    lfo_r: Lfo,
    mod_depth: f32,        // In samples
    mod_rate: f32,         // Hz
    mod_enabled: bool,
    // Saturation
    saturation: f32,       // 0.0 = off, 1.0 = full
}

impl Delay {
    /// Create a new delay effect
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as f32;
        let max_delay = (sr * 2.0) as usize; // 2 seconds max
        
        let mut lfo_r = Lfo::new(sr);
        lfo_r.phase = 0.25; // Offset right LFO for stereo width
        
        Self {
            delay_l: DelayLine::new(max_delay),
            delay_r: DelayLine::new(max_delay),
            time_l: 0.25,
            time_r: 0.25,
            feedback: 0.4,
            cross_feedback: 0.0,
            mix: 0.3,
            sample_rate: sr,
            lfo_l: Lfo::new(sr),
            lfo_r,
            mod_depth: 0.0,
            mod_rate: 0.5,
            mod_enabled: false,
            saturation: 0.0,
        }
    }

    /// Set left delay time in seconds
    pub fn set_time_l(&mut self, time: f32) {
        self.time_l = time.clamp(0.0, 2.0);
    }

    /// Set right delay time in seconds
    pub fn set_time_r(&mut self, time: f32) {
        self.time_r = time.clamp(0.0, 2.0);
    }

    /// Set both channels to same time
    pub fn set_time(&mut self, time: f32) {
        self.set_time_l(time);
        self.set_time_r(time);
    }

    /// Set feedback (0.0 - 0.95)
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback.clamp(0.0, 0.95);
    }

    /// Set cross feedback for ping-pong effect
    pub fn set_cross_feedback(&mut self, cross: f32) {
        self.cross_feedback = cross.clamp(0.0, 0.95);
    }

    /// Set wet/dry mix (0.0 - 1.0)
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Enable ping-pong mode
    pub fn set_ping_pong(&mut self, enabled: bool) {
        if enabled {
            self.cross_feedback = self.feedback;
            self.feedback = 0.0;
        } else {
            self.feedback = self.cross_feedback;
            self.cross_feedback = 0.0;
        }
    }

    /// Sync delay time to tempo
    pub fn sync_to_tempo(&mut self, tempo: f64, division: f32) {
        // division: 1.0 = quarter note, 0.5 = eighth, etc.
        let beat_time = 60.0 / tempo;
        let delay_time = (beat_time * division as f64) as f32;
        self.set_time(delay_time);
    }

    /// Set modulation rate in Hz
    pub fn set_mod_rate(&mut self, rate: f32) {
        self.mod_rate = rate.clamp(0.01, 10.0);
        self.lfo_l.set_rate(rate);
        self.lfo_r.set_rate(rate);
    }

    /// Set modulation depth in milliseconds
    pub fn set_mod_depth(&mut self, depth_ms: f32) {
        self.mod_depth = (depth_ms.clamp(0.0, 20.0) * 0.001 * self.sample_rate);
        self.mod_enabled = depth_ms > 0.0;
    }

    /// Enable/disable modulation
    pub fn set_mod_enabled(&mut self, enabled: bool) {
        self.mod_enabled = enabled;
    }

    /// Set feedback saturation amount (0.0 - 1.0)
    pub fn set_saturation(&mut self, amount: f32) {
        self.saturation = amount.clamp(0.0, 1.0);
    }

    /// Process a stereo sample pair
    pub fn process_sample(&mut self, left: f32, right: f32) -> (f32, f32) {
        let base_delay_l = self.time_l * self.sample_rate;
        let base_delay_r = self.time_r * self.sample_rate;

        // Apply modulation if enabled
        let (delay_samples_l, delay_samples_r) = if self.mod_enabled {
            let mod_l = self.lfo_l.next() * self.mod_depth;
            let mod_r = self.lfo_r.next() * self.mod_depth;
            (
                (base_delay_l + mod_l).max(1.0),
                (base_delay_r + mod_r).max(1.0),
            )
        } else {
            (base_delay_l, base_delay_r)
        };

        let delayed_l = self.delay_l.read_interp(delay_samples_l);
        let delayed_r = self.delay_r.read_interp(delay_samples_r);

        // Calculate feedback with saturation
        let fb_l = delayed_l * self.feedback + delayed_r * self.cross_feedback;
        let fb_r = delayed_r * self.feedback + delayed_l * self.cross_feedback;
        
        let fb_l = saturate(fb_l, self.saturation);
        let fb_r = saturate(fb_r, self.saturation);

        self.delay_l.write(left + fb_l);
        self.delay_r.write(right + fb_r);

        let out_l = left * (1.0 - self.mix) + delayed_l * self.mix;
        let out_r = right * (1.0 - self.mix) + delayed_r * self.mix;

        (out_l, out_r)
    }

    /// Clear delay buffers
    pub fn clear(&mut self) {
        self.delay_l.clear();
        self.delay_r.clear();
        self.lfo_l.reset();
        self.lfo_r.reset();
    }
}

impl Plugin for Delay {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: "pulse.delay".to_string(),
            name: "Stereo Delay".to_string(),
            vendor: "Pulse".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        }
    }

    fn init(&mut self, config: &PluginConfig) -> Result<()> {
        if (config.sample_rate - self.sample_rate).abs() > 1.0 {
            *self = Delay::new(config.sample_rate as u32);
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
        match id {
            0 => self.set_time_l(value * 2.0),      // 0-1 -> 0-2 seconds
            1 => self.set_time_r(value * 2.0),
            2 => self.set_feedback(value * 0.95),
            3 => self.set_mix(value),
            4 => self.set_mod_rate(value * 10.0),   // 0-1 -> 0-10 Hz
            5 => self.set_mod_depth(value * 20.0),  // 0-1 -> 0-20 ms
            6 => self.set_saturation(value),
            _ => {}
        }
    }

    fn get_parameter(&self, id: u32) -> f32 {
        match id {
            0 => self.time_l / 2.0,
            1 => self.time_r / 2.0,
            2 => self.feedback / 0.95,
            3 => self.mix,
            4 => self.mod_rate / 10.0,
            5 => self.mod_depth / (20.0 * 0.001 * self.sample_rate),
            6 => self.saturation,
            _ => 0.0,
        }
    }

    fn tail(&self) -> u32 {
        ((self.time_l.max(self.time_r) * 10.0) * self.sample_rate) as u32
    }

    fn reset(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_new() {
        let delay = Delay::new(44100);
        assert_eq!(delay.time_l, 0.25);
        assert_eq!(delay.saturation, 0.0);
        assert!(!delay.mod_enabled);
    }

    #[test]
    fn test_delay_process() {
        let mut delay = Delay::new(44100);
        delay.set_time(0.005); // 5ms = 220.5 samples
        delay.set_mix(1.0);
        delay.set_feedback(0.0);

        let mut outputs = Vec::new();
        outputs.push(delay.process_sample(1.0, 1.0).0);
        
        for _ in 0..300 {
            outputs.push(delay.process_sample(0.0, 0.0).0);
        }
        
        let has_echo = outputs.iter().skip(1).any(|&x| x > 0.3);
        assert!(has_echo, "No echo found in outputs");
    }

    #[test]
    fn test_delay_feedback() {
        let mut delay = Delay::new(44100);
        delay.set_time(0.005);
        delay.set_feedback(0.7);
        delay.set_mix(1.0);

        delay.process_sample(1.0, 1.0);

        let mut peak_count = 0;
        let mut prev = 0.0;
        for _ in 0..2000 {
            let (l, _) = delay.process_sample(0.0, 0.0);
            if l > 0.05 && l > prev {
                peak_count += 1;
            }
            prev = l;
        }
        assert!(peak_count >= 2, "Expected multiple echoes, got {}", peak_count);
    }

    #[test]
    fn test_delay_ping_pong() {
        let mut delay = Delay::new(44100);
        delay.set_time_l(0.1);
        delay.set_time_r(0.2);
        delay.set_ping_pong(true);
        assert_eq!(delay.feedback, 0.0);
        assert!(delay.cross_feedback > 0.0);
    }

    #[test]
    fn test_delay_tempo_sync() {
        let mut delay = Delay::new(44100);
        delay.sync_to_tempo(120.0, 1.0);
        assert!((delay.time_l - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_delay_modulation() {
        let mut delay = Delay::new(44100);
        delay.set_time(0.01);
        delay.set_mod_rate(2.0);
        delay.set_mod_depth(5.0); // 5ms
        delay.set_mix(1.0);
        delay.set_feedback(0.0);
        
        assert!(delay.mod_enabled);
        
        // Process some samples - modulation should cause pitch variation
        delay.process_sample(1.0, 1.0);
        let mut outputs = Vec::new();
        for _ in 0..1000 {
            outputs.push(delay.process_sample(0.0, 0.0).0);
        }
        
        // Should have non-zero output due to modulated delay
        assert!(outputs.iter().any(|&x| x.abs() > 0.001));
    }

    #[test]
    fn test_delay_saturation() {
        let mut delay = Delay::new(44100);
        delay.set_saturation(1.0);
        
        // Saturate function should soft-clip
        let saturated = saturate(2.0, 1.0);
        assert!(saturated < 2.0);
        assert!(saturated > 0.0);
        
        // No saturation = pass through
        let clean = saturate(2.0, 0.0);
        assert_eq!(clean, 2.0);
    }

    #[test]
    fn test_delay_feedback_with_saturation() {
        let mut delay = Delay::new(44100);
        delay.set_time(0.005);
        delay.set_feedback(0.9);
        delay.set_saturation(0.8);
        delay.set_mix(1.0);

        // Process loud signal
        delay.process_sample(1.0, 1.0);
        
        // With saturation, feedback should stay bounded
        let mut max_val = 0.0f32;
        for _ in 0..5000 {
            let (l, _) = delay.process_sample(0.0, 0.0);
            max_val = max_val.max(l.abs());
        }
        
        // Saturation should prevent runaway
        assert!(max_val < 2.0, "Saturation failed, max={}", max_val);
    }

    #[test]
    fn test_delay_plugin_info() {
        let delay = Delay::new(44100);
        assert_eq!(delay.info().name, "Stereo Delay");
    }

    #[test]
    fn test_delay_parameters() {
        let mut delay = Delay::new(44100);
        
        delay.set_parameter(4, 0.5); // mod rate
        delay.set_parameter(5, 0.5); // mod depth
        delay.set_parameter(6, 0.5); // saturation
        
        assert!((delay.get_parameter(4) - 0.5).abs() < 0.01);
        assert!((delay.get_parameter(6) - 0.5).abs() < 0.01);
    }
}
