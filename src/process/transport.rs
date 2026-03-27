//! Transport state

/// Transport playback state
#[derive(Debug, Clone, Copy, Default)]
pub struct TransportState {
    /// Is playing
    pub playing: bool,
    /// Is recording
    pub recording: bool,
    /// Is looping
    pub looping: bool,
    /// Position in samples
    pub position_samples: i64,
    /// Position in beats
    pub position_beats: f64,
    /// Loop start in beats
    pub loop_start: f64,
    /// Loop end in beats
    pub loop_end: f64,
}

impl TransportState {
    /// Check if position is within loop
    pub fn in_loop(&self) -> bool {
        self.looping && self.position_beats >= self.loop_start && self.position_beats < self.loop_end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_default() {
        let t = TransportState::default();
        assert!(!t.playing);
        assert_eq!(t.position_samples, 0);
    }

    #[test]
    fn test_in_loop() {
        let t = TransportState {
            looping: true,
            position_beats: 2.0,
            loop_start: 1.0,
            loop_end: 4.0,
            ..Default::default()
        };
        assert!(t.in_loop());
    }
}
