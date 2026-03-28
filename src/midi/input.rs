//! MIDI input handling

use super::message::{MidiEvent, MidiMessage};
use crate::Result;
use crossbeam_channel::{bounded, Receiver, Sender};
use midir::{MidiInput as MidirInput, MidiInputConnection};
use parking_lot::Mutex;
use std::sync::Arc;

/// List available MIDI input devices
pub fn list_midi_inputs() -> Result<Vec<String>> {
    let midi_in = MidirInput::new("pulse-list")
        .map_err(|e| crate::Error::Audio(format!("Failed to create MIDI input: {}", e)))?;
    
    let ports: Vec<String> = midi_in.ports()
        .iter()
        .filter_map(|p| midi_in.port_name(p).ok())
        .collect();
    
    Ok(ports)
}

/// MIDI input connection
pub struct MidiInput {
    _connection: MidiInputConnection<()>,
    receiver: Receiver<MidiEvent>,
}

impl MidiInput {
    /// Connect to a MIDI device by name
    pub fn connect(device_name: &str) -> Result<Self> {
        let midi_in = MidirInput::new("pulse")
            .map_err(|e| crate::Error::Audio(format!("Failed to create MIDI input: {}", e)))?;
        
        let port = midi_in.ports()
            .into_iter()
            .find(|p| midi_in.port_name(p).map(|n| n.contains(device_name)).unwrap_or(false))
            .ok_or_else(|| crate::Error::Audio(format!("MIDI device not found: {}", device_name)))?;
        
        let (sender, receiver) = bounded(1024);
        
        let connection = midi_in.connect(
            &port,
            "pulse-input",
            move |timestamp_us, data, _| {
                let message = MidiMessage::parse(data);
                let event = MidiEvent { timestamp_us, message };
                let _ = sender.try_send(event);
            },
            (),
        ).map_err(|e| crate::Error::Audio(format!("Failed to connect: {}", e)))?;
        
        Ok(Self {
            _connection: connection,
            receiver,
        })
    }

    /// Poll for MIDI events (non-blocking)
    pub fn poll(&self) -> Option<MidiEvent> {
        self.receiver.try_recv().ok()
    }

    /// Get all pending events
    pub fn poll_all(&self) -> Vec<MidiEvent> {
        let mut events = Vec::new();
        while let Some(event) = self.poll() {
            events.push(event);
        }
        events
    }
}

/// MIDI input manager for multiple connections
pub struct MidiInputManager {
    inputs: Vec<MidiInput>,
}

impl MidiInputManager {
    pub fn new() -> Self {
        Self { inputs: Vec::new() }
    }

    /// Connect to a device
    pub fn connect(&mut self, device_name: &str) -> Result<()> {
        let input = MidiInput::connect(device_name)?;
        self.inputs.push(input);
        Ok(())
    }

    /// Poll all inputs
    pub fn poll_all(&self) -> Vec<MidiEvent> {
        let mut events = Vec::new();
        for input in &self.inputs {
            events.extend(input.poll_all());
        }
        // Sort by timestamp
        events.sort_by_key(|e| e.timestamp_us);
        events
    }
}

impl Default for MidiInputManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_midi_inputs() {
        // Should not panic
        let _ = list_midi_inputs();
    }

    #[test]
    fn test_midi_manager_new() {
        let manager = MidiInputManager::new();
        assert!(manager.inputs.is_empty());
    }
}
