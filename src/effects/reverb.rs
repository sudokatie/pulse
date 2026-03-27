//! Reverb effect - Freeverb-style implementation

use crate::buffer::AudioBuffer;
use crate::plugin::{Plugin, PluginCategory, PluginConfig, PluginInfo};
use crate::process::ProcessContext;
use crate::Result;

/// Comb filter for reverb
#[derive(Clone)]
struct CombFilter {
    buffer: Vec<f32>,
    index: usize,
    feedback: f32,
    damp1: f32,
    damp2: f32,
    filterstore: f32,
}

impl CombFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            index: 0,
            feedback: 0.5,
            damp1: 0.5,
            damp2: 0.5,
            filterstore: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.index];
        self.filterstore = output * self.damp2 + self.filterstore * self.damp1;
        self.buffer[self.index] = input + self.filterstore * self.feedback;
        self.index = (self.index + 1) % self.buffer.len();
        output
    }

    fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback;
    }

    fn set_damp(&mut self, damp: f32) {
        self.damp1 = damp;
        self.damp2 = 1.0 - damp;
    }
}

/// Allpass filter for reverb
#[derive(Clone)]
struct AllpassFilter {
    buffer: Vec<f32>,
    index: usize,
}

impl AllpassFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            index: 0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let bufout = self.buffer[self.index];
        let output = -input + bufout;
        self.buffer[self.index] = input + bufout * 0.5;
        self.index = (self.index + 1) % self.buffer.len();
        output
    }
}

/// Freeverb-style reverb effect
#[derive(Clone)]
pub struct Reverb {
    combs_l: [CombFilter; 8],
    combs_r: [CombFilter; 8],
    allpasses_l: [AllpassFilter; 4],
    allpasses_r: [AllpassFilter; 4],
    room_size: f32,
    damping: f32,
    wet: f32,
    dry: f32,
    width: f32,
    sample_rate: f32,
}

// Comb filter sizes (tuned for 44100 Hz)
const COMB_SIZES: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS_SIZES: [usize; 4] = [556, 441, 341, 225];
const STEREO_SPREAD: usize = 23;

impl Reverb {
    /// Create a new reverb effect
    pub fn new(sample_rate: u32) -> Self {
        let scale = sample_rate as f32 / 44100.0;
        
        let combs_l = std::array::from_fn(|i| {
            CombFilter::new((COMB_SIZES[i] as f32 * scale) as usize)
        });
        let combs_r = std::array::from_fn(|i| {
            CombFilter::new(((COMB_SIZES[i] + STEREO_SPREAD) as f32 * scale) as usize)
        });
        let allpasses_l = std::array::from_fn(|i| {
            AllpassFilter::new((ALLPASS_SIZES[i] as f32 * scale) as usize)
        });
        let allpasses_r = std::array::from_fn(|i| {
            AllpassFilter::new(((ALLPASS_SIZES[i] + STEREO_SPREAD) as f32 * scale) as usize)
        });

        let mut reverb = Self {
            combs_l,
            combs_r,
            allpasses_l,
            allpasses_r,
            room_size: 0.5,
            damping: 0.5,
            wet: 0.3,
            dry: 0.7,
            width: 1.0,
            sample_rate: sample_rate as f32,
        };
        reverb.update_params();
        reverb
    }

    fn update_params(&mut self) {
        let feedback = self.room_size * 0.28 + 0.7;
        for comb in &mut self.combs_l {
            comb.set_feedback(feedback);
            comb.set_damp(self.damping);
        }
        for comb in &mut self.combs_r {
            comb.set_feedback(feedback);
            comb.set_damp(self.damping);
        }
    }

    /// Set room size (0.0 - 1.0)
    pub fn set_room_size(&mut self, size: f32) {
        self.room_size = size.clamp(0.0, 1.0);
        self.update_params();
    }

    /// Set damping (0.0 - 1.0)
    pub fn set_damping(&mut self, damp: f32) {
        self.damping = damp.clamp(0.0, 1.0);
        self.update_params();
    }

    /// Set wet level (0.0 - 1.0)
    pub fn set_wet(&mut self, wet: f32) {
        self.wet = wet.clamp(0.0, 1.0);
        self.dry = 1.0 - self.wet;
    }

    /// Set stereo width (0.0 - 1.0)
    pub fn set_width(&mut self, width: f32) {
        self.width = width.clamp(0.0, 1.0);
    }

    /// Process a stereo sample pair
    pub fn process_sample(&mut self, left: f32, right: f32) -> (f32, f32) {
        let input = (left + right) * 0.5;
        
        // Parallel comb filters
        let mut out_l = 0.0;
        let mut out_r = 0.0;
        for comb in &mut self.combs_l {
            out_l += comb.process(input);
        }
        for comb in &mut self.combs_r {
            out_r += comb.process(input);
        }

        // Series allpass filters
        for ap in &mut self.allpasses_l {
            out_l = ap.process(out_l);
        }
        for ap in &mut self.allpasses_r {
            out_r = ap.process(out_r);
        }

        // Stereo width
        let wet1 = self.wet * (1.0 + self.width) / 2.0;
        let wet2 = self.wet * (1.0 - self.width) / 2.0;

        let result_l = out_l * wet1 + out_r * wet2 + left * self.dry;
        let result_r = out_r * wet1 + out_l * wet2 + right * self.dry;

        (result_l, result_r)
    }
}

impl Plugin for Reverb {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            id: "pulse.reverb".to_string(),
            name: "Reverb".to_string(),
            vendor: "Pulse".to_string(),
            version: "1.0.0".to_string(),
            category: PluginCategory::Effect,
            inputs: 2,
            outputs: 2,
        }
    }

    fn init(&mut self, config: &PluginConfig) -> Result<()> {
        if (config.sample_rate - self.sample_rate).abs() > 1.0 {
            *self = Reverb::new(config.sample_rate as u32);
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
            0 => self.set_room_size(value),
            1 => self.set_damping(value),
            2 => self.set_wet(value),
            3 => self.set_width(value),
            _ => {}
        }
    }

    fn get_parameter(&self, id: u32) -> f32 {
        match id {
            0 => self.room_size,
            1 => self.damping,
            2 => self.wet,
            3 => self.width,
            _ => 0.0,
        }
    }

    fn tail(&self) -> u32 {
        (self.sample_rate * 3.0) as u32 // ~3 seconds tail
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverb_new() {
        let reverb = Reverb::new(44100);
        assert_eq!(reverb.room_size, 0.5);
    }

    #[test]
    fn test_reverb_process_sample() {
        let mut reverb = Reverb::new(44100);
        let (l, r) = reverb.process_sample(1.0, 1.0);
        assert!(l.is_finite());
        assert!(r.is_finite());
    }

    #[test]
    fn test_reverb_tail() {
        let mut reverb = Reverb::new(44100);
        reverb.set_room_size(0.9);
        reverb.set_wet(1.0);
        
        // Feed impulse
        reverb.process_sample(1.0, 1.0);
        
        // Check for tail
        let mut has_tail = false;
        for _ in 0..44100 {
            let (l, _) = reverb.process_sample(0.0, 0.0);
            if l.abs() > 0.001 {
                has_tail = true;
            }
        }
        assert!(has_tail);
    }

    #[test]
    fn test_reverb_params() {
        let mut reverb = Reverb::new(44100);
        reverb.set_parameter(0, 0.8);
        assert!((reverb.get_parameter(0) - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_reverb_plugin_info() {
        let reverb = Reverb::new(44100);
        assert_eq!(reverb.info().name, "Reverb");
    }

    #[test]
    fn test_reverb_wet_dry() {
        let mut reverb = Reverb::new(44100);
        reverb.set_wet(0.0); // Full dry
        let (l, _) = reverb.process_sample(1.0, 1.0);
        // Should be close to input when fully dry
        assert!(l.abs() > 0.5);
    }
}
