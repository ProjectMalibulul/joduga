//! MIDI input handling.
//!
//! Listens for MIDI events from external devices and queues them
//! for the C++ audio thread via a lock-free ring buffer.

use crate::lockfree_queue::{LockFreeRingBuffer, MIDIEventCmd};
use midir::{Ignore, MidiInput, MidiInputConnection};
use std::error::Error;
use std::sync::Arc;

/// Standard MIDI status nibbles.
#[repr(u32)]
pub enum MidiStatus {
    NoteOff = 0x80,
    NoteOn = 0x90,
    ControlChange = 0xB0,
    PitchBend = 0xE0,
}

/// Owns a `midir` connection for the lifetime of MIDI input.
pub struct MidiInputHandler {
    _connection: Option<MidiInputConnection<()>>,
}

impl MidiInputHandler {
    /// Open the first matching MIDI port and start forwarding events.
    ///
    /// If `port_name` is `None` the first available port is used.
    /// Returns `Ok` with `_connection: None` when no ports exist.
    pub fn new(
        midi_queue: Arc<LockFreeRingBuffer<MIDIEventCmd>>,
        port_name: Option<&str>,
    ) -> Result<Self, Box<dyn Error>> {
        let mut midi_in = MidiInput::new("Joduga MIDI Input")?;
        midi_in.ignore(Ignore::None);

        let ports = midi_in.ports();
        if ports.is_empty() {
            eprintln!("[midi] no MIDI input ports available");
            return Ok(Self { _connection: None });
        }

        let port = match port_name {
            Some(name) => ports
                .iter()
                .find(|p| midi_in.port_name(p).is_ok_and(|n| n.contains(name)))
                .ok_or_else(|| format!("MIDI port containing '{name}' not found"))?,
            None => &ports[0],
        };

        let display = midi_in.port_name(port)?;
        eprintln!("[midi] opening: {display}");

        let conn = midi_in.connect(
            port,
            "joduga-midi",
            move |_ts, msg, _| dispatch(&midi_queue, msg),
            (),
        )?;

        Ok(Self {
            _connection: Some(conn),
        })
    }
}

/// Parse a raw MIDI message and push it into the lock-free queue.
fn dispatch(queue: &LockFreeRingBuffer<MIDIEventCmd>, msg: &[u8]) {
    if msg.is_empty() {
        return;
    }

    let status = msg[0] & 0xF0;
    let cmd = match (status, msg.len()) {
        (0x90, 3..) => MIDIEventCmd {
            event_type: MidiStatus::NoteOn as u32,
            pitch: msg[1] as u32,
            velocity: msg[2] as u32,
            timestamp_samples: 0,
        },
        (0x80, 3..) => MIDIEventCmd {
            event_type: MidiStatus::NoteOff as u32,
            pitch: msg[1] as u32,
            velocity: msg[2] as u32,
            timestamp_samples: 0,
        },
        (0xB0, 3..) => MIDIEventCmd {
            event_type: MidiStatus::ControlChange as u32,
            pitch: msg[1] as u32,    // controller number
            velocity: msg[2] as u32, // controller value
            timestamp_samples: 0,
        },
        (0xE0, 3..) => MIDIEventCmd {
            event_type: MidiStatus::PitchBend as u32,
            pitch: ((msg[2] as u32) << 7) | (msg[1] as u32),
            velocity: 0,
            timestamp_samples: 0,
        },
        _ => return, // ignore sysex, timing, etc.
    };

    let _ = queue.enqueue(cmd);
}
