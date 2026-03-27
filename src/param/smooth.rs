//! Parameter smoothing

/// Parameter smoother using exponential smoothing
#[derive(Debug, Clone)]
pub struct ParamSmoother {
    current: f32,
    target: f32,
    coeff: f32,
    sample_rate: f32,
    time_ms: f32,
}

impl ParamSmoother {
    /// Create a new smoother
    pub fn new(initial: f32, sample_rate: f32, time_ms: f32) -> Self {
        let coeff = Self::calc_coeff(sample_rate, time_ms);
        Self {
            current: initial,
            target: initial,
            coeff,
            sample_rate,
            time_ms,
        }
    }

    fn calc_coeff(sample_rate: f32, time_ms: f32) -> f32 {
        if time_ms <= 0.0 {
            1.0
        } else {
            1.0 - (-1.0 / (time_ms * 0.001 * sample_rate)).exp()
        }
    }

    /// Set new target value
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// Set target immediately (no smoothing)
    pub fn set_immediate(&mut self, value: f32) {
        self.current = value;
        self.target = value;
    }

    /// Get next smoothed value
    pub fn next(&mut self) -> f32 {
        self.current += self.coeff * (self.target - self.current);
        self.current
    }

    /// Check if smoother has reached target
    pub fn is_settled(&self) -> bool {
        (self.current - self.target).abs() < 1e-6
    }

    /// Get current value without advancing
    pub fn current(&self) -> f32 {
        self.current
    }

    /// Get target value
    pub fn target(&self) -> f32 {
        self.target
    }

    /// Set smoothing time
    pub fn set_time(&mut self, time_ms: f32) {
        self.time_ms = time_ms;
        self.coeff = Self::calc_coeff(self.sample_rate, time_ms);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smoother_new() {
        let s = ParamSmoother::new(0.5, 44100.0, 10.0);
        assert_eq!(s.current(), 0.5);
        assert_eq!(s.target(), 0.5);
    }

    #[test]
    fn test_smoother_immediate() {
        let mut s = ParamSmoother::new(0.0, 44100.0, 10.0);
        s.set_immediate(1.0);
        assert_eq!(s.current(), 1.0);
        assert!(s.is_settled());
    }

    #[test]
    fn test_smoother_converges() {
        let mut s = ParamSmoother::new(0.0, 44100.0, 5.0);
        s.set_target(1.0);
        
        // Process for a while
        for _ in 0..4410 { // 100ms at 44.1kHz
            s.next();
        }
        
        // Should have converged
        assert!((s.current() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_smoother_zero_time() {
        let mut s = ParamSmoother::new(0.0, 44100.0, 0.0);
        s.set_target(1.0);
        s.next();
        assert_eq!(s.current(), 1.0);
    }
}
