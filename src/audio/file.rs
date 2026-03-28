//! Audio file reading and writing

use crate::buffer::AudioBuffer;
use crate::Result;
use hound::{WavReader, WavWriter, WavSpec, SampleFormat};
use std::path::Path;

/// Audio file metadata
#[derive(Debug, Clone)]
pub struct AudioFile {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub duration_samples: usize,
}

/// Read an audio file into an AudioBuffer
pub fn read_audio_file<P: AsRef<Path>>(path: P) -> Result<(AudioBuffer, AudioFile)> {
    let reader = WavReader::open(path.as_ref())
        .map_err(|e| crate::Error::Audio(format!("Failed to open file: {}", e)))?;
    
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels;
    let bits_per_sample = spec.bits_per_sample;
    
    let samples: Vec<f32> = match spec.sample_format {
        SampleFormat::Float => {
            reader.into_samples::<f32>()
                .map(|s| s.unwrap_or(0.0))
                .collect()
        }
        SampleFormat::Int => {
            let max_val = (1 << (bits_per_sample - 1)) as f32;
            reader.into_samples::<i32>()
                .map(|s| s.unwrap_or(0) as f32 / max_val)
                .collect()
        }
    };
    
    let frames = samples.len() / channels as usize;
    let buffer = AudioBuffer::from_interleaved(&samples, channels as usize);
    
    let info = AudioFile {
        sample_rate,
        channels,
        bits_per_sample,
        duration_samples: frames,
    };
    
    Ok((buffer, info))
}

/// Write an AudioBuffer to an audio file
pub fn write_audio_file<P: AsRef<Path>>(
    path: P,
    buffer: &AudioBuffer,
    sample_rate: u32,
) -> Result<()> {
    let spec = WavSpec {
        channels: buffer.channels() as u16,
        sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    
    let mut writer = WavWriter::create(path.as_ref(), spec)
        .map_err(|e| crate::Error::Audio(format!("Failed to create file: {}", e)))?;
    
    let interleaved = buffer.to_interleaved();
    for sample in interleaved {
        writer.write_sample(sample)
            .map_err(|e| crate::Error::Audio(format!("Failed to write sample: {}", e)))?;
    }
    
    writer.finalize()
        .map_err(|e| crate::Error::Audio(format!("Failed to finalize file: {}", e)))?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_write_and_read() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.wav");
        
        // Create a buffer with some data
        let mut buffer = AudioBuffer::new(2, 1024);
        for i in 0..1024 {
            let t = i as f32 / 44100.0;
            let sample = (t * 440.0 * std::f32::consts::TAU).sin() * 0.5;
            buffer.channel_mut(0).unwrap()[i] = sample;
            buffer.channel_mut(1).unwrap()[i] = sample;
        }
        
        // Write
        write_audio_file(&path, &buffer, 44100).unwrap();
        
        // Read back
        let (read_buffer, info) = read_audio_file(&path).unwrap();
        
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.channels, 2);
        assert_eq!(read_buffer.frames(), 1024);
        
        // Check first sample matches
        let orig = buffer.channel(0).unwrap()[0];
        let read = read_buffer.channel(0).unwrap()[0];
        assert!((orig - read).abs() < 0.0001);
    }

    #[test]
    fn test_read_nonexistent() {
        let result = read_audio_file("/nonexistent/file.wav");
        assert!(result.is_err());
    }
}
