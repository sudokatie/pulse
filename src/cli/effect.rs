//! Effect CLI - process audio with built-in effects

use std::path::Path;
use hound::{WavReader, WavWriter, WavSpec, SampleFormat};
use crate::buffer::AudioBuffer;
use crate::effects::{Reverb, Delay, Compressor, ParametricEQ, Distortion};
use crate::plugin::Plugin;
use crate::process::ProcessContext;

/// Available built-in effects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectType {
    Reverb,
    Delay,
    Compressor,
    Eq,
    Distortion,
}

impl EffectType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "reverb" | "rev" => Some(Self::Reverb),
            "delay" | "dly" => Some(Self::Delay),
            "compressor" | "comp" => Some(Self::Compressor),
            "eq" | "parametric-eq" | "peq" => Some(Self::Eq),
            "distortion" | "dist" => Some(Self::Distortion),
            _ => None,
        }
    }
    
    pub fn name(&self) -> &'static str {
        match self {
            Self::Reverb => "reverb",
            Self::Delay => "delay",
            Self::Compressor => "compressor",
            Self::Eq => "eq",
            Self::Distortion => "distortion",
        }
    }
    
    pub fn description(&self) -> &'static str {
        match self {
            Self::Reverb => "Freeverb-style reverb with room size and damping",
            Self::Delay => "Stereo delay with tempo sync and ping-pong mode",
            Self::Compressor => "Dynamics compressor with soft knee",
            Self::Eq => "4-band parametric EQ",
            Self::Distortion => "Waveshaping distortion with multiple modes",
        }
    }
    
    pub fn parameters(&self) -> &'static [(&'static str, &'static str, f32, f32, f32)] {
        // (name, description, min, max, default)
        match self {
            Self::Reverb => &[
                ("room_size", "Room size (0-1)", 0.0, 1.0, 0.5),
                ("damping", "High frequency damping (0-1)", 0.0, 1.0, 0.5),
                ("wet", "Wet/dry mix (0-1)", 0.0, 1.0, 0.3),
                ("width", "Stereo width (0-1)", 0.0, 1.0, 1.0),
            ],
            Self::Delay => &[
                ("time", "Delay time in seconds", 0.001, 2.0, 0.25),
                ("feedback", "Feedback amount (0-1)", 0.0, 0.99, 0.3),
                ("wet", "Wet/dry mix (0-1)", 0.0, 1.0, 0.5),
                ("ping_pong", "Ping-pong mode (0=off, 1=on)", 0.0, 1.0, 0.0),
            ],
            Self::Compressor => &[
                ("threshold", "Threshold in dB", -60.0, 0.0, -20.0),
                ("ratio", "Compression ratio", 1.0, 20.0, 4.0),
                ("attack", "Attack time in ms", 0.1, 100.0, 10.0),
                ("release", "Release time in ms", 10.0, 1000.0, 100.0),
                ("makeup", "Makeup gain in dB", 0.0, 24.0, 0.0),
            ],
            Self::Eq => &[
                ("low_gain", "Low band gain in dB", -12.0, 12.0, 0.0),
                ("low_freq", "Low band frequency", 20.0, 500.0, 100.0),
                ("mid_gain", "Mid band gain in dB", -12.0, 12.0, 0.0),
                ("mid_freq", "Mid band frequency", 200.0, 5000.0, 1000.0),
                ("high_gain", "High band gain in dB", -12.0, 12.0, 0.0),
                ("high_freq", "High band frequency", 2000.0, 20000.0, 8000.0),
            ],
            Self::Distortion => &[
                ("drive", "Drive amount (0-1)", 0.0, 1.0, 0.5),
                ("tone", "Tone control (0-1)", 0.0, 1.0, 0.5),
                ("mix", "Wet/dry mix (0-1)", 0.0, 1.0, 1.0),
                ("mode", "Distortion mode (0=soft, 1=hard, 2=foldback)", 0.0, 2.0, 0.0),
            ],
        }
    }
}

/// Process an audio file with a built-in effect
pub fn process_effect(
    effect_type: EffectType,
    input_path: &Path,
    output_path: &Path,
    params: &[(String, f32)],
) -> Result<ProcessResult, String> {
    // Read input file
    let mut reader = WavReader::open(input_path)
        .map_err(|e| format!("Failed to open input file: {}", e))?;
    
    let spec = reader.spec();
    let sample_rate = spec.sample_rate as f32;
    let channels = spec.channels as usize;
    
    if channels > 2 {
        return Err("Only mono and stereo files supported".into());
    }
    
    // Read all samples
    let samples: Vec<f32> = match spec.sample_format {
        SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
        SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            let scale = 1.0 / (1u32 << (bits - 1)) as f32;
            reader.samples::<i32>().map(|s| s.unwrap() as f32 * scale).collect()
        }
    };
    
    // Create buffer from interleaved samples
    let mut buffer = AudioBuffer::from_interleaved(&samples, channels);
    let num_frames = buffer.frames();
    
    // Create and configure effect
    let context = ProcessContext::new(sample_rate);
    
    match effect_type {
        EffectType::Reverb => {
            let mut effect = Reverb::new(sample_rate as u32);
            apply_params(&mut effect, params);
            effect.process(&mut buffer, &context);
        }
        EffectType::Delay => {
            let mut effect = Delay::new(sample_rate as u32);
            apply_params(&mut effect, params);
            effect.process(&mut buffer, &context);
        }
        EffectType::Compressor => {
            let mut effect = Compressor::new(sample_rate as u32);
            apply_params(&mut effect, params);
            effect.process(&mut buffer, &context);
        }
        EffectType::Eq => {
            let mut effect = ParametricEQ::new(sample_rate as u32);
            apply_params(&mut effect, params);
            effect.process(&mut buffer, &context);
        }
        EffectType::Distortion => {
            let mut effect = Distortion::new(sample_rate as u32);
            apply_params(&mut effect, params);
            effect.process(&mut buffer, &context);
        }
    }
    
    // Interleave output
    let output_samples = buffer.to_interleaved();
    
    // Write output file
    let out_spec = WavSpec {
        channels: channels as u16,
        sample_rate: spec.sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    
    let mut writer = WavWriter::create(output_path, out_spec)
        .map_err(|e| format!("Failed to create output file: {}", e))?;
    
    for sample in output_samples {
        writer.write_sample(sample)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }
    
    writer.finalize()
        .map_err(|e| format!("Failed to finalize output: {}", e))?;
    
    Ok(ProcessResult {
        input_path: input_path.to_path_buf(),
        output_path: output_path.to_path_buf(),
        sample_rate: sample_rate as u32,
        channels: channels as u32,
        frames: num_frames as u64,
        effect: effect_type.name().to_string(),
    })
}

fn apply_params<P: Plugin>(effect: &mut P, params: &[(String, f32)]) {
    let param_info = effect.parameters();
    for (name, value) in params {
        // Try to find matching parameter by name
        for info in &param_info {
            if info.name.to_lowercase() == name.to_lowercase() 
                || info.name.to_lowercase().replace(' ', "_") == name.to_lowercase() 
            {
                effect.set_parameter(info.id, *value);
                break;
            }
        }
    }
}

/// Result of processing an audio file
#[derive(Debug)]
pub struct ProcessResult {
    pub input_path: std::path::PathBuf,
    pub output_path: std::path::PathBuf,
    pub sample_rate: u32,
    pub channels: u32,
    pub frames: u64,
    pub effect: String,
}

impl ProcessResult {
    pub fn duration_secs(&self) -> f32 {
        self.frames as f32 / self.sample_rate as f32
    }
}

/// List all available effects
pub fn list_effects() -> Vec<(EffectType, &'static str, &'static str)> {
    vec![
        (EffectType::Reverb, "reverb", EffectType::Reverb.description()),
        (EffectType::Delay, "delay", EffectType::Delay.description()),
        (EffectType::Compressor, "compressor", EffectType::Compressor.description()),
        (EffectType::Eq, "eq", EffectType::Eq.description()),
        (EffectType::Distortion, "distortion", EffectType::Distortion.description()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_effect_type_from_str() {
        assert_eq!(EffectType::from_str("reverb"), Some(EffectType::Reverb));
        assert_eq!(EffectType::from_str("REVERB"), Some(EffectType::Reverb));
        assert_eq!(EffectType::from_str("rev"), Some(EffectType::Reverb));
        assert_eq!(EffectType::from_str("delay"), Some(EffectType::Delay));
        assert_eq!(EffectType::from_str("comp"), Some(EffectType::Compressor));
        assert_eq!(EffectType::from_str("unknown"), None);
    }
    
    #[test]
    fn test_effect_type_parameters() {
        let params = EffectType::Reverb.parameters();
        assert!(!params.is_empty());
        assert!(params.iter().any(|(name, _, _, _, _)| *name == "room_size"));
    }
    
    #[test]
    fn test_list_effects() {
        let effects = list_effects();
        assert_eq!(effects.len(), 5);
    }
}
