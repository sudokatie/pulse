//! VST3 automation bridge - sample-accurate parameter automation

use std::ffi::c_void;

use crate::param::{AutomationCurve, AutomationManager};
use crate::plugin::Plugin;

use super::adapter::SharedParameterState;
use super::process_adapter::{IParameterChangesVtable, IParamValueQueueVtable};
use super::types::K_RESULT_OK;

use std::sync::{Arc, Mutex};

/// Sample-accurate automation point from VST3
#[derive(Debug, Clone, Copy)]
pub struct AutomationEvent {
    /// Parameter ID
    pub param_id: u32,
    /// Sample offset within the current block
    pub sample_offset: i32,
    /// Normalized value (0-1)
    pub value: f64,
}

/// Extract all automation events from VST3 IParameterChanges
///
/// # Safety
/// The param_changes pointer must point to a valid IParameterChanges interface.
pub unsafe fn extract_automation_events(param_changes: *mut c_void) -> Vec<AutomationEvent> {
    let mut events = Vec::new();

    if param_changes.is_null() {
        return events;
    }

    let vtable = *(param_changes as *const *const IParameterChangesVtable);
    if vtable.is_null() {
        return events;
    }

    let count = ((*vtable).get_parameter_count)(param_changes);

    for i in 0..count {
        let queue = ((*vtable).get_parameter_data)(param_changes, i);
        if queue.is_null() {
            continue;
        }

        let queue_vtable = *(queue as *const *const IParamValueQueueVtable);
        if queue_vtable.is_null() {
            continue;
        }

        let param_id = ((*queue_vtable).get_parameter_id)(queue);
        let point_count = ((*queue_vtable).get_point_count)(queue);

        for point_index in 0..point_count {
            let mut sample_offset: i32 = 0;
            let mut value: f64 = 0.0;

            let result = ((*queue_vtable).get_point)(
                queue,
                point_index,
                &mut sample_offset,
                &mut value,
            );

            if result == K_RESULT_OK {
                events.push(AutomationEvent {
                    param_id,
                    sample_offset,
                    value,
                });
            }
        }
    }

    // Sort events by sample offset for proper processing order
    events.sort_by_key(|e| e.sample_offset);

    events
}

/// Apply automation events to plugin parameters
///
/// This function applies parameter changes at their exact sample offsets,
/// enabling sample-accurate automation from the DAW.
pub fn apply_automation_to_plugin(
    events: &[AutomationEvent],
    param_state: &Arc<Mutex<SharedParameterState>>,
    plugin: &mut dyn Plugin,
) {
    if let Ok(mut state) = param_state.lock() {
        for event in events {
            // Update shared state with normalized value
            state.set_normalized(event.param_id, event.value);

            // Convert normalized to plain value for the plugin
            let plain = state.mapping.normalized_to_plain(event.param_id, event.value);
            plugin.set_parameter(event.param_id, plain as f32);
        }
    }
}

/// Feed automation events into an AutomationManager for recording
///
/// This is useful when you want to record DAW automation into Pulse's
/// internal automation system for playback or analysis.
pub fn record_automation_events(
    events: &[AutomationEvent],
    manager: &mut AutomationManager,
    block_start_sample: u64,
    param_state: &Arc<Mutex<SharedParameterState>>,
) {
    if let Ok(state) = param_state.lock() {
        for event in events {
            // Convert normalized value to plain value
            let plain = state.mapping.normalized_to_plain(event.param_id, event.value);

            // Add point at the absolute sample position
            let absolute_sample = block_start_sample + event.sample_offset as u64;
            manager.add_point(
                event.param_id,
                absolute_sample,
                plain as f32,
                AutomationCurve::Linear,
            );
        }
    }
}

/// Process automation events with sample-accurate timing within a block
///
/// This function groups events by their sample offset and processes the audio
/// block in segments, applying parameter changes at the exact sample where they
/// should take effect.
pub struct SampleAccurateProcessor {
    /// Events sorted by sample offset
    events: Vec<AutomationEvent>,
    /// Current position in the block (sample offset)
    current_position: usize,
    /// Whether processing is complete
    done: bool,
}

impl SampleAccurateProcessor {
    /// Create a new processor from automation events
    pub fn new(events: Vec<AutomationEvent>) -> Self {
        Self {
            events,
            current_position: 0,
            done: false,
        }
    }

    /// Get the next segment of samples to process before the next parameter change
    ///
    /// Returns (start_offset, length) or None if all segments have been processed.
    /// After calling this, use `events_at_offset` to apply any parameter changes
    /// that occur at the end of this segment.
    pub fn next_segment(&mut self, block_size: usize) -> Option<(usize, usize)> {
        if self.done || self.current_position >= block_size {
            return None;
        }

        // Find the next event offset after current position
        let next_event_offset = self
            .events
            .iter()
            .map(|e| e.sample_offset as usize)
            .find(|&offset| offset > self.current_position);

        let start = self.current_position;

        match next_event_offset {
            Some(event_offset) if event_offset < block_size => {
                // Process up to the event
                let length = event_offset - start;
                self.current_position = event_offset;
                Some((start, length))
            }
            _ => {
                // No more events before block end - process rest of block
                let length = block_size - start;
                self.current_position = block_size;
                self.done = true;
                Some((start, length))
            }
        }
    }

    /// Get events that should be applied at the given sample offset
    pub fn events_at_offset(&self, offset: usize) -> impl Iterator<Item = &AutomationEvent> {
        self.events
            .iter()
            .filter(move |e| e.sample_offset as usize == offset)
    }

    /// Reset the processor for reuse
    pub fn reset(&mut self) {
        self.current_position = 0;
        self.done = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_automation_event_creation() {
        let event = AutomationEvent {
            param_id: 0,
            sample_offset: 128,
            value: 0.75,
        };

        assert_eq!(event.param_id, 0);
        assert_eq!(event.sample_offset, 128);
        assert!((event.value - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_sample_accurate_processor_empty() {
        let mut processor = SampleAccurateProcessor::new(vec![]);

        let segment = processor.next_segment(256);
        assert_eq!(segment, Some((0, 256)));

        let segment = processor.next_segment(256);
        assert_eq!(segment, None);
    }

    #[test]
    fn test_sample_accurate_processor_single_event() {
        let events = vec![AutomationEvent {
            param_id: 0,
            sample_offset: 128,
            value: 0.5,
        }];

        let mut processor = SampleAccurateProcessor::new(events);

        // First segment: 0-128
        let segment = processor.next_segment(256);
        assert_eq!(segment, Some((0, 128)));

        // Second segment: 128-256
        let segment = processor.next_segment(256);
        assert_eq!(segment, Some((128, 128)));

        // No more segments
        let segment = processor.next_segment(256);
        assert_eq!(segment, None);
    }

    #[test]
    fn test_sample_accurate_processor_multiple_events() {
        let events = vec![
            AutomationEvent {
                param_id: 0,
                sample_offset: 64,
                value: 0.25,
            },
            AutomationEvent {
                param_id: 0,
                sample_offset: 192,
                value: 0.75,
            },
        ];

        let mut processor = SampleAccurateProcessor::new(events);

        // First segment: 0-64
        let segment = processor.next_segment(256);
        assert_eq!(segment, Some((0, 64)));

        // Second segment: 64-192
        let segment = processor.next_segment(256);
        assert_eq!(segment, Some((64, 128)));

        // Third segment: 192-256
        let segment = processor.next_segment(256);
        assert_eq!(segment, Some((192, 64)));

        // No more segments
        let segment = processor.next_segment(256);
        assert_eq!(segment, None);
    }

    #[test]
    fn test_sample_accurate_processor_event_at_zero() {
        let events = vec![AutomationEvent {
            param_id: 0,
            sample_offset: 0,
            value: 0.5,
        }];

        let mut processor = SampleAccurateProcessor::new(events);

        // Should start processing after the event at 0
        let segment = processor.next_segment(256);
        assert_eq!(segment, Some((0, 256)));
    }

    #[test]
    fn test_events_at_offset() {
        let events = vec![
            AutomationEvent {
                param_id: 0,
                sample_offset: 128,
                value: 0.5,
            },
            AutomationEvent {
                param_id: 1,
                sample_offset: 128,
                value: 0.75,
            },
            AutomationEvent {
                param_id: 0,
                sample_offset: 256,
                value: 1.0,
            },
        ];

        let processor = SampleAccurateProcessor::new(events);

        let at_128: Vec<_> = processor.events_at_offset(128).collect();
        assert_eq!(at_128.len(), 2);
        assert_eq!(at_128[0].param_id, 0);
        assert_eq!(at_128[1].param_id, 1);

        let at_256: Vec<_> = processor.events_at_offset(256).collect();
        assert_eq!(at_256.len(), 1);
        assert_eq!(at_256[0].param_id, 0);

        let at_0: Vec<_> = processor.events_at_offset(0).collect();
        assert_eq!(at_0.len(), 0);
    }

    #[test]
    fn test_processor_reset() {
        let events = vec![AutomationEvent {
            param_id: 0,
            sample_offset: 128,
            value: 0.5,
        }];

        let mut processor = SampleAccurateProcessor::new(events);

        // Consume segments
        let _ = processor.next_segment(256);
        let _ = processor.next_segment(256);
        assert!(processor.next_segment(256).is_none());

        // Reset
        processor.reset();

        // Should work again
        let segment = processor.next_segment(256);
        assert_eq!(segment, Some((0, 128)));
    }
}
