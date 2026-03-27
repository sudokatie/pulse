//! Distortion/waveshaping effect

use crate::buffer::AudioBuffer;
use crate::effects::filter::{BiquadFilter, FilterType};
use crate::plugin::{Plugin, PluginCategory, PluginConfig, PluginInfo};
use crate::process::ProcessContext;
use crate::Result;

fn fast_tanh(x: f32) -> f32 {
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

/// Distortion types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DistortionType {
    SoftClip,
    HardClip,
    Foldback,
    Tube,
    Bitcrush,
}

/// Distortion effect
#[derive(Clone)]
pub struct Distortion {
    drive: f32,
    tone: f32,
    mix: f32,
    dist_type: DistortionType,
    pre_filter: BiquadFilter,
    post_filter: BiquadFilter,
    sample_rate: f32,
}

impl Distortion {
    /// Create a new distortion effect
    pub fn new(sample_rate: u32) -> Self {
        let mut dist = Self {
            drive: 1.0,
            tone: 0.5,
            mix: 1.0,
            dist_type: DistortionType::SoftClip,
            pre_filter: BiquadFilter::new(),
            post_filter: BiquadFilter::new(),
            sample_rate: sample_rate as f32,
        };
        dist.update_filters();
        dist
    }

    fn update_filters(&mut self) {
        // Pre-filter: slight high boost for presence
        self.pre_filter.set_coefficients(
            FilterType::HighShelf, 
            3000.0, 
            0.707, 
            3.0, 
            self.sample_rate
        );
        
        // Post-filter: tone control
        let cutoff = 1000.0 + self.tone * 9000.0;
        self.post_filter.set_coefficients(
            FilterType::LowPass, 
            cutoff, 
            0.707, 
            0.0, 
            self.sample_rate
        );
    }

    /// Set drive (1.0 - 100.0)
    pub fn set_drive(&mut self, drive: f32) {
        self.drive = drive.clamp(1.0, 100.0);
    }

    /// Set tone (0.0 - 1.0)
    pub fn set_tone(&mut self, tone: f32) {
        self.tone = tone.clamp(0.0, 1.0);
        self.update_filters();
    }

    /// Set wet/dry mix (0.0 - 1.0)
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Set distortion type
    pub fn set_type(&mut self, dist_type: DistortionType) {
        self.dist_type = dist_type;
    }

    fn waveshape(&self, input: f32) -> f32 {
        let driven = input * self.drive;

        match self.dist_type {
            DistortionType::SoftClip => fast_tanh(driven),
            DistortionType::HardClip => driven.clamp(-1.0, 1.0),
            DistortionType::Foldback => {
                let mut x = driven;
                while x > 1.0 || x < -1.0 {
                    if x > 1.0 { x = 2.0 - x; }
                    if x < -1.0 { x = -2.0 - x; }
                }
                x
            }
            DistortionType::Tube => {
                // Asymmetric soft clipping (tube-like)
                if driven >= 0.0 {
                    1.0 - (-driven).exp()
                } else {
                    -1.0 + (driven * 0.5).exp()
                }
            }
            DistortionType::Bitcrush => {
                let bits = 16.0 - (self.drive - 1.0) * 0.15;
                let bits = bits.clamp(2.0, 16.0);
                let levels = 2.0_f32.powf(bits);
                (driven * levels).round() / levels
            }
        }
    }

    /// Process a mono sample
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let pre = self.pre_filter.process(input);
        let distorted = self.waveshape(pre);
        let post = self.post_filter.process(distorted);
        
        // Compensate for drive gain
        let compensated = post / self.drive.sqrt();
        
        input * (1.0 - self.mix) + compensated * self.mix
    }

    /// Process stereo
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // For stereo, we process mid-side to avoid phase issues
        let mid = (left + right) * 0.5;
        let side = (left - right) * 0.5;
        
        let mid_dist = self.process_sample(mid);
        
        (mid_dist + side, mid_dist - side)
    }
}

impl Plugin for Distortion {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: "pulse.distortion".to_string(),
            name: "Distortion".to_string(),
            vendor: "Pulse".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        }
    }

    fn init(&mut self, config: &PluginConfig) -> Result<()> {
        self.sample_rate = config.sample_rate;
        self.update_filters();
        Ok(())
    }

    fn process(&mut self, buffer: &mut AudioBuffer, _ctx: &ProcessContext) {
        let frames = buffer.frames();
        if buffer.channels() < 2 {
            if let Some(ch) = buffer.channel_mut(0) {
                for sample in ch.iter_mut() {
                    *sample = self.process_sample(*sample);
                }
            }
            return;
        }

        for i in 0..frames {
            let left = buffer.channel(0).map(|c| c[i]).unwrap_or(0.0);
            let right = buffer.channel(1).map(|c| c[i]).unwrap_or(0.0);
            let (out_l, out_r) = self.process_stereo(left, right);
            if let Some(ch) = buffer.channel_mut(0) { ch[i] = out_l; }
            if let Some(ch) = buffer.channel_mut(1) { ch[i] = out_r; }
        }
    }

    fn set_parameter(&mut self, id: u32, value: f32) {
        match id {
            0 => self.set_drive(1.0 + value * 99.0), // 1-100
            1 => self.set_tone(value),
            2 => self.set_mix(value),
            3 => {
                let types = [
                    DistortionType::SoftClip,
                    DistortionType::HardClip,
                    DistortionType::Foldback,
                    DistortionType::Tube,
                    DistortionType::Bitcrush,
                ];
                let idx = (value * 4.0).round() as usize;
                self.set_type(types[idx.min(4)]);
            }
            _ => {}
        }
    }

    fn get_parameter(&self, id: u32) -> f32 {
        match id {
            0 => (self.drive - 1.0) / 99.0,
            1 => self.tone,
            2 => self.mix,
            3 => match self.dist_type {
                DistortionType::SoftClip => 0.0,
                DistortionType::HardClip => 0.25,
                DistortionType::Foldback => 0.5,
                DistortionType::Tube => 0.75,
                DistortionType::Bitcrush => 1.0,
            },
            _ => 0.0,
        }
    }

    fn reset(&mut self) {
        self.pre_filter.reset();
        self.post_filter.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distortion_new() {
        let dist = Distortion::new(44100);
        assert_eq!(dist.drive, 1.0);
    }

    #[test]
    fn test_distortion_soft_clip() {
        let mut dist = Distortion::new(44100);
        dist.set_drive(10.0);
        dist.set_type(DistortionType::SoftClip);
        dist.set_mix(1.0);
        
        let out = dist.process_sample(0.5);
        assert!(out.abs() <= 1.5); // Some headroom
    }

    #[test]
    fn test_distortion_hard_clip() {
        let mut dist = Distortion::new(44100);
        dist.set_drive(10.0);
        dist.set_type(DistortionType::HardClip);
        dist.set_mix(1.0);
        
        for _ in 0..10 {
            let out = dist.process_sample(0.5);
            assert!(out.abs() <= 1.5);
        }
    }

    #[test]
    fn test_distortion_foldback() {
        let mut dist = Distortion::new(44100);
        dist.set_drive(5.0);
        dist.set_type(DistortionType::Foldback);
        dist.set_mix(1.0);
        
        let out = dist.process_sample(0.8);
        assert!(out.is_finite());
    }

    #[test]
    fn test_distortion_tube() {
        let mut dist = Distortion::new(44100);
        dist.set_type(DistortionType::Tube);
        let out = dist.process_sample(0.5);
        assert!(out.is_finite());
    }

    #[test]
    fn test_distortion_bitcrush() {
        let mut dist = Distortion::new(44100);
        dist.set_drive(50.0);
        dist.set_type(DistortionType::Bitcrush);
        let out = dist.process_sample(0.5);
        assert!(out.is_finite());
    }

    #[test]
    fn test_distortion_mix() {
        let mut dist = Distortion::new(44100);
        dist.set_drive(10.0);
        dist.set_mix(0.0); // Fully dry
        
        let out = dist.process_sample(0.5);
        assert!((out - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_distortion_plugin_info() {
        let dist = Distortion::new(44100);
        assert_eq!(dist.info().name, "Distortion");
    }
}
