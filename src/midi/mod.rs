//! MIDI input handling

mod input;
mod message;

pub use input::{MidiInput, MidiInputManager, list_midi_inputs};
pub use message::{MidiMessage, MidiEvent};
