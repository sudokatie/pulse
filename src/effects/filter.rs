//! Biquad filter implementation

/// Filter types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterType {
    LowPass,
    HighPass,
    BandPass,
    Notch,
    Peak,
    LowShelf,
    HighShelf,
}

/// Biquad filter
#[derive(Debug, Clone)]
pub struct BiquadFilter {
    // Coefficients
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    // State
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Default for BiquadFilter {
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }
}

impl BiquadFilter {
    /// Create new filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate coefficients for given filter type
    pub fn set_coefficients(&mut self, filter_type: FilterType, freq: f32, q: f32, gain_db: f32, sample_rate: f32) {
        let omega = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let sin_w = omega.sin();
        let cos_w = omega.cos();
        let alpha = sin_w / (2.0 * q);
        let a = 10.0_f32.powf(gain_db / 40.0);

        let (b0, b1, b2, a0, a1, a2) = match filter_type {
            FilterType::LowPass => {
                let b0 = (1.0 - cos_w) / 2.0;
                let b1 = 1.0 - cos_w;
                let b2 = (1.0 - cos_w) / 2.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::HighPass => {
                let b0 = (1.0 + cos_w) / 2.0;
                let b1 = -(1.0 + cos_w);
                let b2 = (1.0 + cos_w) / 2.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::BandPass => {
                let b0 = alpha;
                let b1 = 0.0;
                let b2 = -alpha;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Notch => {
                let b0 = 1.0;
                let b1 = -2.0 * cos_w;
                let b2 = 1.0;
                let a0 = 1.0 + alpha;
                let a1 = -2.0 * cos_w;
                let a2 = 1.0 - alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::Peak => {
                let b0 = 1.0 + alpha * a;
                let b1 = -2.0 * cos_w;
                let b2 = 1.0 - alpha * a;
                let a0 = 1.0 + alpha / a;
                let a1 = -2.0 * cos_w;
                let a2 = 1.0 - alpha / a;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::LowShelf => {
                let sqrt_a = a.sqrt();
                let b0 = a * ((a + 1.0) - (a - 1.0) * cos_w + 2.0 * sqrt_a * alpha);
                let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w);
                let b2 = a * ((a + 1.0) - (a - 1.0) * cos_w - 2.0 * sqrt_a * alpha);
                let a0 = (a + 1.0) + (a - 1.0) * cos_w + 2.0 * sqrt_a * alpha;
                let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cos_w);
                let a2 = (a + 1.0) + (a - 1.0) * cos_w - 2.0 * sqrt_a * alpha;
                (b0, b1, b2, a0, a1, a2)
            }
            FilterType::HighShelf => {
                let sqrt_a = a.sqrt();
                let b0 = a * ((a + 1.0) + (a - 1.0) * cos_w + 2.0 * sqrt_a * alpha);
                let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w);
                let b2 = a * ((a + 1.0) + (a - 1.0) * cos_w - 2.0 * sqrt_a * alpha);
                let a0 = (a + 1.0) - (a - 1.0) * cos_w + 2.0 * sqrt_a * alpha;
                let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cos_w);
                let a2 = (a + 1.0) - (a - 1.0) * cos_w - 2.0 * sqrt_a * alpha;
                (b0, b1, b2, a0, a1, a2)
            }
        };

        // Normalize coefficients
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    /// Process a single sample
    pub fn process(&mut self, input: f32) -> f32 {
        let output = self.b0 * input + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1 - self.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }

    /// Reset filter state
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_default() {
        let f = BiquadFilter::new();
        // Unity gain passthrough
        assert_eq!(f.b0, 1.0);
    }

    #[test]
    fn test_lowpass_coefficients() {
        let mut f = BiquadFilter::new();
        f.set_coefficients(FilterType::LowPass, 1000.0, 0.707, 0.0, 44100.0);
        assert!(f.b0.is_finite());
    }

    #[test]
    fn test_filter_process() {
        let mut f = BiquadFilter::new();
        f.set_coefficients(FilterType::LowPass, 5000.0, 0.707, 0.0, 44100.0);
        
        // Process some samples
        for i in 0..100 {
            let input = (i as f32 * 0.1).sin();
            let output = f.process(input);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_filter_reset() {
        let mut f = BiquadFilter::new();
        f.process(1.0);
        f.reset();
        assert_eq!(f.x1, 0.0);
        assert_eq!(f.y1, 0.0);
    }

    #[test]
    fn test_peak_filter() {
        let mut f = BiquadFilter::new();
        f.set_coefficients(FilterType::Peak, 1000.0, 1.0, 6.0, 44100.0);
        assert!(f.b0.is_finite());
    }

    #[test]
    fn test_shelf_filters() {
        let mut f = BiquadFilter::new();
        f.set_coefficients(FilterType::LowShelf, 200.0, 0.707, 6.0, 44100.0);
        assert!(f.b0.is_finite());
        
        f.set_coefficients(FilterType::HighShelf, 8000.0, 0.707, 6.0, 44100.0);
        assert!(f.b0.is_finite());
    }
}
