//! Audio buffer for processing

/// Audio buffer with multiple channels
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    data: Vec<Vec<f32>>,
    frames: usize,
}

impl AudioBuffer {
    /// Create a new audio buffer
    pub fn new(channels: usize, frames: usize) -> Self {
        Self {
            data: vec![vec![0.0; frames]; channels],
            frames,
        }
    }

    /// Create from interleaved samples
    pub fn from_interleaved(samples: &[f32], channels: usize) -> Self {
        let frames = samples.len() / channels;
        let mut data = vec![vec![0.0; frames]; channels];
        
        for (i, sample) in samples.iter().enumerate() {
            let channel = i % channels;
            let frame = i / channels;
            if frame < frames {
                data[channel][frame] = *sample;
            }
        }
        
        Self { data, frames }
    }

    /// Convert to interleaved samples
    pub fn to_interleaved(&self) -> Vec<f32> {
        let channels = self.channels();
        let mut output = vec![0.0; self.frames * channels];
        
        for frame in 0..self.frames {
            for (ch, channel_data) in self.data.iter().enumerate() {
                output[frame * channels + ch] = channel_data[frame];
            }
        }
        
        output
    }

    /// Get number of channels
    pub fn channels(&self) -> usize {
        self.data.len()
    }

    /// Get number of frames
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// Get a channel slice
    pub fn channel(&self, index: usize) -> Option<&[f32]> {
        self.data.get(index).map(|v| v.as_slice())
    }

    /// Get a mutable channel slice
    pub fn channel_mut(&mut self, index: usize) -> Option<&mut [f32]> {
        self.data.get_mut(index).map(|v| v.as_mut_slice())
    }

    /// Get all channels as slices
    pub fn channels_slice(&self) -> Vec<&[f32]> {
        self.data.iter().map(|v| v.as_slice()).collect()
    }

    /// Clear buffer to zeros
    pub fn clear(&mut self) {
        for channel in &mut self.data {
            channel.fill(0.0);
        }
    }

    /// Copy from another buffer
    pub fn copy_from(&mut self, other: &AudioBuffer) {
        let channels = self.channels().min(other.channels());
        let frames = self.frames.min(other.frames);
        
        for ch in 0..channels {
            self.data[ch][..frames].copy_from_slice(&other.data[ch][..frames]);
        }
    }

    /// Add another buffer (mix)
    pub fn add(&mut self, other: &AudioBuffer) {
        let channels = self.channels().min(other.channels());
        let frames = self.frames.min(other.frames);
        
        for ch in 0..channels {
            for i in 0..frames {
                self.data[ch][i] += other.data[ch][i];
            }
        }
    }

    /// Scale all samples by a factor
    pub fn scale(&mut self, factor: f32) {
        for channel in &mut self.data {
            for sample in channel.iter_mut() {
                *sample *= factor;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_new() {
        let buf = AudioBuffer::new(2, 256);
        assert_eq!(buf.channels(), 2);
        assert_eq!(buf.frames(), 256);
    }

    #[test]
    fn test_buffer_from_interleaved() {
        let interleaved = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let buf = AudioBuffer::from_interleaved(&interleaved, 2);
        
        assert_eq!(buf.channels(), 2);
        assert_eq!(buf.frames(), 3);
        assert_eq!(buf.channel(0).unwrap(), &[1.0, 3.0, 5.0]);
        assert_eq!(buf.channel(1).unwrap(), &[2.0, 4.0, 6.0]);
    }

    #[test]
    fn test_buffer_to_interleaved() {
        let interleaved = vec![1.0, 2.0, 3.0, 4.0];
        let buf = AudioBuffer::from_interleaved(&interleaved, 2);
        let result = buf.to_interleaved();
        assert_eq!(result, interleaved);
    }

    #[test]
    fn test_buffer_channel_access() {
        let mut buf = AudioBuffer::new(2, 4);
        buf.channel_mut(0).unwrap()[0] = 1.0;
        assert_eq!(buf.channel(0).unwrap()[0], 1.0);
    }

    #[test]
    fn test_buffer_clear() {
        let mut buf = AudioBuffer::from_interleaved(&[1.0, 2.0, 3.0, 4.0], 2);
        buf.clear();
        assert!(buf.channel(0).unwrap().iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_buffer_scale() {
        let mut buf = AudioBuffer::from_interleaved(&[1.0, 2.0], 1);
        buf.scale(2.0);
        assert_eq!(buf.channel(0).unwrap(), &[2.0, 4.0]);
    }
}
