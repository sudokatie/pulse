//! MIDI message types

/// MIDI message types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MidiMessage {
    NoteOn { channel: u8, note: u8, velocity: u8 },
    NoteOff { channel: u8, note: u8, velocity: u8 },
    ControlChange { channel: u8, controller: u8, value: u8 },
    ProgramChange { channel: u8, program: u8 },
    PitchBend { channel: u8, value: i16 },
    Aftertouch { channel: u8, pressure: u8 },
    PolyAftertouch { channel: u8, note: u8, pressure: u8 },
    Unknown,
}

/// MIDI event with timestamp
#[derive(Debug, Clone)]
pub struct MidiEvent {
    pub timestamp_us: u64,
    pub message: MidiMessage,
}

impl MidiMessage {
    /// Parse MIDI bytes into a message
    pub fn parse(data: &[u8]) -> Self {
        if data.is_empty() {
            return MidiMessage::Unknown;
        }

        let status = data[0];
        let channel = status & 0x0F;
        let msg_type = status & 0xF0;

        match msg_type {
            0x90 if data.len() >= 3 => {
                if data[2] == 0 {
                    MidiMessage::NoteOff { channel, note: data[1], velocity: 0 }
                } else {
                    MidiMessage::NoteOn { channel, note: data[1], velocity: data[2] }
                }
            }
            0x80 if data.len() >= 3 => {
                MidiMessage::NoteOff { channel, note: data[1], velocity: data[2] }
            }
            0xB0 if data.len() >= 3 => {
                MidiMessage::ControlChange { channel, controller: data[1], value: data[2] }
            }
            0xC0 if data.len() >= 2 => {
                MidiMessage::ProgramChange { channel, program: data[1] }
            }
            0xE0 if data.len() >= 3 => {
                let value = ((data[2] as i16) << 7 | data[1] as i16) - 8192;
                MidiMessage::PitchBend { channel, value }
            }
            0xD0 if data.len() >= 2 => {
                MidiMessage::Aftertouch { channel, pressure: data[1] }
            }
            0xA0 if data.len() >= 3 => {
                MidiMessage::PolyAftertouch { channel, note: data[1], pressure: data[2] }
            }
            _ => MidiMessage::Unknown,
        }
    }

    /// Get note number if this is a note message
    pub fn note(&self) -> Option<u8> {
        match self {
            MidiMessage::NoteOn { note, .. } => Some(*note),
            MidiMessage::NoteOff { note, .. } => Some(*note),
            MidiMessage::PolyAftertouch { note, .. } => Some(*note),
            _ => None,
        }
    }

    /// Get velocity if this is a note on message
    pub fn velocity(&self) -> Option<u8> {
        match self {
            MidiMessage::NoteOn { velocity, .. } => Some(*velocity),
            MidiMessage::NoteOff { velocity, .. } => Some(*velocity),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_note_on() {
        let msg = MidiMessage::parse(&[0x90, 60, 100]);
        assert!(matches!(msg, MidiMessage::NoteOn { channel: 0, note: 60, velocity: 100 }));
    }

    #[test]
    fn test_parse_note_off() {
        let msg = MidiMessage::parse(&[0x80, 60, 64]);
        assert!(matches!(msg, MidiMessage::NoteOff { channel: 0, note: 60, velocity: 64 }));
    }

    #[test]
    fn test_parse_note_on_zero_velocity() {
        let msg = MidiMessage::parse(&[0x90, 60, 0]);
        assert!(matches!(msg, MidiMessage::NoteOff { channel: 0, note: 60, velocity: 0 }));
    }

    #[test]
    fn test_parse_cc() {
        let msg = MidiMessage::parse(&[0xB0, 1, 127]);
        assert!(matches!(msg, MidiMessage::ControlChange { channel: 0, controller: 1, value: 127 }));
    }

    #[test]
    fn test_parse_pitch_bend() {
        let msg = MidiMessage::parse(&[0xE0, 0, 64]); // Center
        assert!(matches!(msg, MidiMessage::PitchBend { channel: 0, value: 0 }));
    }

    #[test]
    fn test_note_accessor() {
        let msg = MidiMessage::NoteOn { channel: 0, note: 60, velocity: 100 };
        assert_eq!(msg.note(), Some(60));
    }
}
