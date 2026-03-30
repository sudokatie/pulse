//! Sample-accurate parameter automation

use std::collections::BTreeMap;

/// A single automation point
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutomationPoint {
    /// Sample position
    pub sample: u64,
    /// Parameter value
    pub value: f32,
    /// Curve type for interpolation to next point
    pub curve: AutomationCurve,
}

/// Automation curve types
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AutomationCurve {
    /// Jump to value instantly
    Step,
    /// Linear interpolation to next point
    #[default]
    Linear,
    /// Exponential curve (good for frequency/volume)
    Exponential,
    /// S-curve (smooth transitions)
    SCurve,
}

impl AutomationCurve {
    /// Interpolate between two values
    pub fn interpolate(&self, from: f32, to: f32, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            AutomationCurve::Step => from,
            AutomationCurve::Linear => from + (to - from) * t,
            AutomationCurve::Exponential => {
                // Exponential interpolation (good for dB, frequency)
                if from <= 0.0 || to <= 0.0 {
                    // Fall back to linear if values <= 0
                    from + (to - from) * t
                } else {
                    from * (to / from).powf(t)
                }
            }
            AutomationCurve::SCurve => {
                // Smooth S-curve using smoothstep
                let t = t * t * (3.0 - 2.0 * t);
                from + (to - from) * t
            }
        }
    }
}

/// Automation lane for a single parameter
#[derive(Debug, Clone, Default)]
pub struct AutomationLane {
    /// Parameter ID
    pub param_id: u32,
    /// Automation points sorted by sample position
    points: BTreeMap<u64, AutomationPoint>,
    /// Current playback position
    position: u64,
    /// Is recording enabled
    recording: bool,
    /// Last recorded value (for change detection)
    last_recorded: Option<f32>,
}

impl AutomationLane {
    /// Create a new automation lane
    pub fn new(param_id: u32) -> Self {
        Self {
            param_id,
            points: BTreeMap::new(),
            position: 0,
            recording: false,
            last_recorded: None,
        }
    }

    /// Add an automation point
    pub fn add_point(&mut self, sample: u64, value: f32, curve: AutomationCurve) {
        self.points.insert(sample, AutomationPoint { sample, value, curve });
    }

    /// Remove an automation point
    pub fn remove_point(&mut self, sample: u64) -> Option<AutomationPoint> {
        self.points.remove(&sample)
    }

    /// Clear all points
    pub fn clear(&mut self) {
        self.points.clear();
    }

    /// Get value at a specific sample position
    pub fn value_at(&self, sample: u64) -> Option<f32> {
        if self.points.is_empty() {
            return None;
        }

        // Find surrounding points
        let before = self.points.range(..=sample).next_back();
        let after = self.points.range(sample..).next();

        match (before, after) {
            (Some((_, p1)), Some((_, p2))) if p1.sample != p2.sample => {
                // Interpolate between points
                let t = (sample - p1.sample) as f32 / (p2.sample - p1.sample) as f32;
                Some(p1.curve.interpolate(p1.value, p2.value, t))
            }
            (Some((_, p)), _) => Some(p.value),
            (None, Some((_, p))) => Some(p.value),
            (None, None) => None,
        }
    }

    /// Get values for a range of samples (for block processing)
    pub fn values_for_block(&self, start: u64, count: usize) -> Vec<Option<f32>> {
        (0..count)
            .map(|i| self.value_at(start + i as u64))
            .collect()
    }

    /// Start recording
    pub fn start_recording(&mut self) {
        self.recording = true;
        self.last_recorded = None;
    }

    /// Stop recording
    pub fn stop_recording(&mut self) {
        self.recording = false;
        self.last_recorded = None;
    }

    /// Record a value (only stores if changed)
    pub fn record(&mut self, sample: u64, value: f32, threshold: f32) {
        if !self.recording {
            return;
        }

        let should_record = match self.last_recorded {
            Some(last) => (value - last).abs() > threshold,
            None => true,
        };

        if should_record {
            self.add_point(sample, value, AutomationCurve::Linear);
            self.last_recorded = Some(value);
        }
    }

    /// Get number of points
    pub fn point_count(&self) -> usize {
        self.points.len()
    }

    /// Get all points as a slice
    pub fn points(&self) -> impl Iterator<Item = &AutomationPoint> {
        self.points.values()
    }

    /// Set playback position
    pub fn set_position(&mut self, sample: u64) {
        self.position = sample;
    }

    /// Get current position
    pub fn position(&self) -> u64 {
        self.position
    }
}

/// Automation manager for multiple parameters
#[derive(Debug, Clone, Default)]
pub struct AutomationManager {
    /// Lanes by parameter ID
    lanes: BTreeMap<u32, AutomationLane>,
    /// Current sample position
    position: u64,
    /// Playback state
    playing: bool,
    /// Recording state
    recording: bool,
    /// Sample rate
    sample_rate: f32,
}

impl AutomationManager {
    /// Create a new automation manager
    pub fn new(sample_rate: f32) -> Self {
        Self {
            lanes: BTreeMap::new(),
            position: 0,
            playing: false,
            recording: false,
            sample_rate,
        }
    }

    /// Get or create a lane for a parameter
    pub fn lane(&mut self, param_id: u32) -> &mut AutomationLane {
        let recording = self.recording;
        let position = self.position;
        self.lanes.entry(param_id).or_insert_with(|| {
            let mut lane = AutomationLane::new(param_id);
            lane.set_position(position);
            if recording {
                lane.start_recording();
            }
            lane
        })
    }

    /// Get a lane if it exists
    pub fn get_lane(&self, param_id: u32) -> Option<&AutomationLane> {
        self.lanes.get(&param_id)
    }

    /// Get value for a parameter at current position
    pub fn value(&self, param_id: u32) -> Option<f32> {
        self.lanes.get(&param_id)?.value_at(self.position)
    }

    /// Get value for a parameter at specific sample
    pub fn value_at(&self, param_id: u32, sample: u64) -> Option<f32> {
        self.lanes.get(&param_id)?.value_at(sample)
    }

    /// Process a block of samples, returning parameter changes
    pub fn process_block(&mut self, frames: usize) -> Vec<(u64, u32, f32)> {
        let mut changes = Vec::new();

        if !self.playing {
            return changes;
        }

        for (&param_id, lane) in &self.lanes {
            let mut last_value: Option<f32> = None;
            
            for i in 0..frames {
                let sample = self.position + i as u64;
                if let Some(value) = lane.value_at(sample) {
                    // Only emit change if value actually changed
                    let emit = match last_value {
                        Some(lv) => (value - lv).abs() > 1e-6,
                        None => true,
                    };
                    if emit {
                        changes.push((sample, param_id, value));
                        last_value = Some(value);
                    }
                }
            }
        }

        self.position += frames as u64;
        changes
    }

    /// Record a parameter value
    pub fn record(&mut self, param_id: u32, value: f32) {
        if self.recording {
            let position = self.position;
            self.lane(param_id).record(position, value, 0.001);
        }
    }

    /// Add a point directly
    pub fn add_point(&mut self, param_id: u32, sample: u64, value: f32, curve: AutomationCurve) {
        self.lane(param_id).add_point(sample, value, curve);
    }

    /// Start playback
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Stop playback
    pub fn stop(&mut self) {
        self.playing = false;
    }

    /// Start recording
    pub fn start_recording(&mut self) {
        self.recording = true;
        for lane in self.lanes.values_mut() {
            lane.start_recording();
        }
    }

    /// Stop recording
    pub fn stop_recording(&mut self) {
        self.recording = false;
        for lane in self.lanes.values_mut() {
            lane.stop_recording();
        }
    }

    /// Set position in samples
    pub fn set_position(&mut self, sample: u64) {
        self.position = sample;
        for lane in self.lanes.values_mut() {
            lane.set_position(sample);
        }
    }

    /// Set position in seconds
    pub fn set_position_seconds(&mut self, seconds: f64) {
        self.set_position((seconds * self.sample_rate as f64) as u64);
    }

    /// Get current position
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Get position in seconds
    pub fn position_seconds(&self) -> f64 {
        self.position as f64 / self.sample_rate as f64
    }

    /// Clear all automation
    pub fn clear(&mut self) {
        self.lanes.clear();
        self.position = 0;
    }

    /// Clear automation for a specific parameter
    pub fn clear_param(&mut self, param_id: u32) {
        if let Some(lane) = self.lanes.get_mut(&param_id) {
            lane.clear();
        }
    }

    /// Get total number of automation points
    pub fn total_points(&self) -> usize {
        self.lanes.values().map(|l| l.point_count()).sum()
    }

    /// Check if playing
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Check if recording
    pub fn is_recording(&self) -> bool {
        self.recording
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_automation_curve_step() {
        let curve = AutomationCurve::Step;
        assert_eq!(curve.interpolate(0.0, 1.0, 0.5), 0.0);
    }

    #[test]
    fn test_automation_curve_linear() {
        let curve = AutomationCurve::Linear;
        assert!((curve.interpolate(0.0, 1.0, 0.5) - 0.5).abs() < 0.001);
        assert!((curve.interpolate(0.0, 1.0, 0.0) - 0.0).abs() < 0.001);
        assert!((curve.interpolate(0.0, 1.0, 1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_automation_curve_exponential() {
        let curve = AutomationCurve::Exponential;
        // Exponential interpolation
        let mid = curve.interpolate(1.0, 100.0, 0.5);
        assert!(mid > 1.0 && mid < 100.0);
        // Should be geometric mean
        assert!((mid - 10.0).abs() < 0.1);
    }

    #[test]
    fn test_automation_curve_scurve() {
        let curve = AutomationCurve::SCurve;
        let mid = curve.interpolate(0.0, 1.0, 0.5);
        assert!((mid - 0.5).abs() < 0.001);
        // S-curve should be slower at edges
        let early = curve.interpolate(0.0, 1.0, 0.1);
        let late = curve.interpolate(0.0, 1.0, 0.9);
        assert!(early < 0.1);
        assert!(late > 0.9);
    }

    #[test]
    fn test_lane_add_point() {
        let mut lane = AutomationLane::new(0);
        lane.add_point(0, 0.0, AutomationCurve::Linear);
        lane.add_point(100, 1.0, AutomationCurve::Linear);
        assert_eq!(lane.point_count(), 2);
    }

    #[test]
    fn test_lane_value_at() {
        let mut lane = AutomationLane::new(0);
        lane.add_point(0, 0.0, AutomationCurve::Linear);
        lane.add_point(100, 1.0, AutomationCurve::Linear);

        assert!((lane.value_at(0).unwrap() - 0.0).abs() < 0.001);
        assert!((lane.value_at(50).unwrap() - 0.5).abs() < 0.001);
        assert!((lane.value_at(100).unwrap() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_lane_value_before_first() {
        let mut lane = AutomationLane::new(0);
        lane.add_point(100, 0.5, AutomationCurve::Linear);
        // Before first point should return first point's value
        assert!((lane.value_at(0).unwrap() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_lane_value_after_last() {
        let mut lane = AutomationLane::new(0);
        lane.add_point(0, 0.0, AutomationCurve::Linear);
        lane.add_point(100, 0.5, AutomationCurve::Linear);
        // After last point should return last point's value
        assert!((lane.value_at(200).unwrap() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_lane_recording() {
        let mut lane = AutomationLane::new(0);
        lane.start_recording();
        lane.record(0, 0.0, 0.01);
        lane.record(100, 0.5, 0.01);
        lane.record(101, 0.501, 0.01); // Should be ignored (below threshold)
        lane.record(200, 1.0, 0.01);
        lane.stop_recording();

        assert_eq!(lane.point_count(), 3);
    }

    #[test]
    fn test_manager_basic() {
        let mut manager = AutomationManager::new(44100.0);
        manager.add_point(0, 0, 0.0, AutomationCurve::Linear);
        manager.add_point(0, 44100, 1.0, AutomationCurve::Linear);

        assert!((manager.value_at(0, 0).unwrap() - 0.0).abs() < 0.001);
        assert!((manager.value_at(0, 22050).unwrap() - 0.5).abs() < 0.001);
        assert!((manager.value_at(0, 44100).unwrap() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_manager_process_block() {
        let mut manager = AutomationManager::new(44100.0);
        manager.add_point(0, 0, 0.0, AutomationCurve::Step);
        manager.add_point(0, 128, 1.0, AutomationCurve::Step);
        manager.play();

        let changes = manager.process_block(256);
        assert!(!changes.is_empty());
        
        // Should have change at sample 0 and 128
        let change_samples: Vec<_> = changes.iter().map(|(s, _, _)| *s).collect();
        assert!(change_samples.contains(&0));
        assert!(change_samples.contains(&128));
    }

    #[test]
    fn test_manager_recording() {
        let mut manager = AutomationManager::new(44100.0);
        manager.start_recording();
        manager.record(0, 0.5);
        manager.set_position(100);
        manager.record(0, 0.8);
        manager.stop_recording();

        assert_eq!(manager.get_lane(0).unwrap().point_count(), 2);
    }

    #[test]
    fn test_manager_position_seconds() {
        let mut manager = AutomationManager::new(44100.0);
        manager.set_position_seconds(1.0);
        assert_eq!(manager.position(), 44100);
        assert!((manager.position_seconds() - 1.0).abs() < 0.001);
    }
}
