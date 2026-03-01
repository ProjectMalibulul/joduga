use crate::lockfree_queue::{LockFreeRingBuffer, MIDIEventCmd};
/// MIDI input handling.
/// Listens for MIDI events from external devices and queues them for the audio engine.
use midir::{Ignore, MidiInput, MidiInputConnection};
use std::error::Error;
use std::sync::Arc;

/// MIDI event types
#[repr(u32)]
pub enum MIDIEventType {
    NoteOn = 0x90,
    NoteOff = 0x80,
    ControlChange = 0xB0,
    PitchBend = 0xE0,
}

/// MIDI input handler
pub struct MIDIInputHandler {
    _connection: Option<MidiInputConnection<()>>,
}

impl MIDIInputHandler {
    /// Initialize MIDI input and start listening.
    ///
    /// # Arguments:
    /// - midi_queue: Shared lock-free queue to push MIDI events to
    /// - port_name: Optional MIDI port name (if None, uses first available)
    ///
    /// # Returns:
    /// Ok(MIDIInputHandler) on success, Err on failure
    pub fn new(
        midi_queue: Arc<LockFreeRingBuffer<MIDIEventCmd>>,
        port_name: Option<&str>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut midi_in = MidiInput::new("Joduga MIDI Input")?;
        midi_in.ignore(Ignore::None);

        // List available MIDI ports
        let in_ports = midi_in.ports();
        if in_ports.is_empty() {
            println!("No MIDI input ports available");
            return Ok(MIDIInputHandler { _connection: None });
        }

        // Select port
        let port = if let Some(name) = port_name {
            in_ports
                .iter()
                .find(|p| {
                    midi_in
                        .port_name(p)
                        .map(|n| n.contains(name))
                        .unwrap_or(false)
                })
                .ok_or("MIDI port not found")?
        } else {
            &in_ports[0]
        };

        let port_name = midi_in.port_name(port)?;
        println!("Opening MIDI port: {}", port_name);

        // Open MIDI connection
        let connection = midi_in.connect(
            port,
            "joduga-midi",
            move |_timestamp, message, _| {
                Self::handle_midi_message(&midi_queue, message);
            },
            (),
        )?;

        Ok(MIDIInputHandler {
            _connection: Some(connection),
        })
    }

    /// Parse and queue a MIDI message
    fn handle_midi_message(queue: &Arc<LockFreeRingBuffer<MIDIEventCmd>>, message: &[u8]) {
        if message.is_empty() {
            return;
        }

        let status = message[0];
        let event_type = status & 0xF0;

        match event_type {
            0x90 => {
                // Note On
                if message.len() >= 3 {
                    let pitch = message[1] as u32;
                    let velocity = message[2] as u32;

                    let cmd = MIDIEventCmd {
                        event_type: MIDIEventType::NoteOn as u32,
                        pitch,
                        velocity,
                        timestamp_samples: 0, // TODO: Use proper timestamp
                    };

                    let _ = queue.enqueue(cmd);
                }
            }
            0x80 => {
                // Note Off
                if message.len() >= 3 {
                    let pitch = message[1] as u32;
                    let velocity = message[2] as u32;

                    let cmd = MIDIEventCmd {
                        event_type: MIDIEventType::NoteOff as u32,
                        pitch,
                        velocity,
                        timestamp_samples: 0,
                    };

                    let _ = queue.enqueue(cmd);
                }
            }
            0xB0 => {
                // Control Change
                if message.len() >= 3 {
                    let controller = message[1] as u32;
                    let value = message[2] as u32;

                    let cmd = MIDIEventCmd {
                        event_type: MIDIEventType::ControlChange as u32,
                        pitch: controller,
                        velocity: value,
                        timestamp_samples: 0,
                    };

                    let _ = queue.enqueue(cmd);
                }
            }
            0xE0 => {
                // Pitch Bend
                if message.len() >= 3 {
                    let lsb = message[1] as u32;
                    let msb = message[2] as u32;
                    let value = (msb << 7) | lsb;

                    let cmd = MIDIEventCmd {
                        event_type: MIDIEventType::PitchBend as u32,
                        pitch: value,
                        velocity: 0,
                        timestamp_samples: 0,
                    };

                    let _ = queue.enqueue(cmd);
                }
            }
            _ => {
                // Ignore other MIDI messages
            }
        }
    }
}
