//! Stereo delay effect with tempo sync

use crate::buffer::AudioBuffer;
use crate::plugin::{Plugin, PluginCategory, PluginConfig, PluginInfo};
use crate::process::ProcessContext;
use crate::Result;

/// Delay line buffer
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

/// Stereo delay effect
#[derive(Clone)]
pub struct Delay {
    delay_l: DelayLine,
    delay_r: DelayLine,
    time_l: f32,       // seconds
    time_r: f32,       // seconds
    feedback: f32,
    cross_feedback: f32, // For ping-pong
    mix: f32,
    sample_rate: f32,
}

impl Delay {
    /// Create a new delay effect
    pub fn new(sample_rate: u32) -> Self {
        let max_delay = (sample_rate as f32 * 2.0) as usize; // 2 seconds max
        Self {
            delay_l: DelayLine::new(max_delay),
            delay_r: DelayLine::new(max_delay),
            time_l: 0.25,
            time_r: 0.25,
            feedback: 0.4,
            cross_feedback: 0.0,
            mix: 0.3,
            sample_rate: sample_rate as f32,
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

    /// Process a stereo sample pair
    pub fn process_sample(&mut self, left: f32, right: f32) -> (f32, f32) {
        let delay_samples_l = self.time_l * self.sample_rate;
        let delay_samples_r = self.time_r * self.sample_rate;

        let delayed_l = self.delay_l.read_interp(delay_samples_l);
        let delayed_r = self.delay_r.read_interp(delay_samples_r);

        // Normal feedback + cross feedback (ping-pong)
        self.delay_l.write(left + delayed_l * self.feedback + delayed_r * self.cross_feedback);
        self.delay_r.write(right + delayed_r * self.feedback + delayed_l * self.cross_feedback);

        let out_l = left * (1.0 - self.mix) + delayed_l * self.mix;
        let out_r = right * (1.0 - self.mix) + delayed_r * self.mix;

        (out_l, out_r)
    }

    /// Clear delay buffers
    pub fn clear(&mut self) {
        self.delay_l.clear();
        self.delay_r.clear();
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
            0 => self.set_time_l(value * 2.0),  // 0-1 -> 0-2 seconds
            1 => self.set_time_r(value * 2.0),
            2 => self.set_feedback(value * 0.95),
            3 => self.set_mix(value),
            _ => {}
        }
    }

    fn get_parameter(&self, id: u32) -> f32 {
        match id {
            0 => self.time_l / 2.0,
            1 => self.time_r / 2.0,
            2 => self.feedback / 0.95,
            3 => self.mix,
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
    }

    #[test]
    fn test_delay_process() {
        let mut delay = Delay::new(44100);
        delay.set_time(0.005); // 5ms = 220.5 samples
        delay.set_mix(1.0);
        delay.set_feedback(0.0);

        // Send impulse and process for delay time
        let mut outputs = Vec::new();
        outputs.push(delay.process_sample(1.0, 1.0).0);
        
        for _ in 0..300 {
            outputs.push(delay.process_sample(0.0, 0.0).0);
        }
        
        // Should have an echo somewhere in the outputs
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

        // With feedback, we should get multiple echoes
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
        delay.sync_to_tempo(120.0, 1.0); // Quarter note at 120 BPM = 0.5s
        assert!((delay.time_l - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_delay_plugin_info() {
        let delay = Delay::new(44100);
        assert_eq!(delay.info().name, "Stereo Delay");
    }
}
