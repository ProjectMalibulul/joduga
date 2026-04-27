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

        Ok(Self { _connection: Some(conn) })
    }
}

/// Parse a raw MIDI message into a queue command, or `None` if the
/// message is empty/malformed/unsupported.
///
/// Per the MIDI 1.0 spec, `NoteOn` with velocity 0 is the
/// running-status form of `NoteOff`; nearly every keyboard uses it,
/// so we translate it here rather than letting downstream nodes
/// re-trigger a still-held note.
pub(crate) fn parse(msg: &[u8]) -> Option<MIDIEventCmd> {
    if msg.is_empty() {
        return None;
    }

    let status = msg[0] & 0xF0;
    // MIDI 1.0 spec: data bytes have the high bit clear (range 0–127).
    // Misbehaving devices and bridged MIDI from buggy software
    // occasionally send data bytes with the high bit set, which would
    // otherwise smear into the C++ side's pitch / velocity registers
    // (e.g. an OOB note number or a pitch-bend value > 16383). Mask
    // defensively so the queue never carries an out-of-spec value
    // regardless of upstream behaviour.
    let d1 = msg.get(1).copied().unwrap_or(0) & 0x7F;
    let d2 = msg.get(2).copied().unwrap_or(0) & 0x7F;
    match (status, msg.len()) {
        (0x90, 3..) => {
            let velocity = d2 as u32;
            // NoteOn vel=0 is canonical NoteOff.
            let event_type =
                if velocity == 0 { MidiStatus::NoteOff as u32 } else { MidiStatus::NoteOn as u32 };
            Some(MIDIEventCmd { event_type, pitch: d1 as u32, velocity, timestamp_samples: 0 })
        }
        (0x80, 3..) => Some(MIDIEventCmd {
            event_type: MidiStatus::NoteOff as u32,
            pitch: d1 as u32,
            velocity: d2 as u32,
            timestamp_samples: 0,
        }),
        (0xB0, 3..) => Some(MIDIEventCmd {
            event_type: MidiStatus::ControlChange as u32,
            pitch: d1 as u32,    // controller number
            velocity: d2 as u32, // controller value
            timestamp_samples: 0,
        }),
        (0xE0, 3..) => Some(MIDIEventCmd {
            event_type: MidiStatus::PitchBend as u32,
            pitch: ((d2 as u32) << 7) | (d1 as u32),
            velocity: 0,
            timestamp_samples: 0,
        }),
        _ => None, // ignore sysex, timing, channel pressure, etc.
    }
}

/// Parse a raw MIDI message and push it into the lock-free queue.
fn dispatch(queue: &LockFreeRingBuffer<MIDIEventCmd>, msg: &[u8]) {
    if let Some(cmd) = parse(msg) {
        // Queue full: drop and log. A status-register counter would be
        // better but that change touches the FFI ABI; logged as
        // future work in .agent/next.md.
        if queue.enqueue(cmd).is_err() {
            eprintln!("[midi] queue full, dropping event");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_note_on_with_velocity_zero_is_note_off() {
        // The single most common MIDI quirk: keyboards emit
        // 0x90 pitch 0x00 instead of 0x80 pitch vel for note-off.
        let cmd = parse(&[0x90, 60, 0]).expect("must parse");
        assert_eq!(cmd.event_type, MidiStatus::NoteOff as u32);
        assert_eq!(cmd.pitch, 60);
        assert_eq!(cmd.velocity, 0);
    }

    #[test]
    fn parse_real_note_on_kept_as_note_on() {
        let cmd = parse(&[0x90, 64, 100]).expect("must parse");
        assert_eq!(cmd.event_type, MidiStatus::NoteOn as u32);
        assert_eq!(cmd.pitch, 64);
        assert_eq!(cmd.velocity, 100);
    }

    #[test]
    fn parse_note_off_kept_as_note_off() {
        let cmd = parse(&[0x80, 64, 50]).expect("must parse");
        assert_eq!(cmd.event_type, MidiStatus::NoteOff as u32);
    }

    #[test]
    fn parse_control_change() {
        let cmd = parse(&[0xB1, 7, 100]).expect("must parse");
        assert_eq!(cmd.event_type, MidiStatus::ControlChange as u32);
        assert_eq!(cmd.pitch, 7);
        assert_eq!(cmd.velocity, 100);
    }

    #[test]
    fn parse_pitch_bend_packs_14_bits_lsb_first() {
        // MIDI pitch bend: data1 = LSB (7 bits), data2 = MSB (7 bits).
        // Center = 8192 = 0x2000 → MSB=0x40, LSB=0x00.
        let cmd = parse(&[0xE0, 0x00, 0x40]).expect("must parse");
        assert_eq!(cmd.event_type, MidiStatus::PitchBend as u32);
        assert_eq!(cmd.pitch, 8192);
    }

    #[test]
    fn parse_empty_message_returns_none() {
        assert!(parse(&[]).is_none());
    }

    #[test]
    fn parse_truncated_note_on_returns_none() {
        // 0x90 needs 2 data bytes; only 1 supplied.
        assert!(parse(&[0x90, 60]).is_none());
    }

    #[test]
    fn parse_sysex_returns_none() {
        // 0xF0 is system exclusive; we deliberately ignore it.
        assert!(parse(&[0xF0, 0x7E, 0x7F]).is_none());
    }

    #[test]
    fn parse_strips_high_bit_on_malformed_data_bytes() {
        // Some devices (especially via flaky USB-MIDI bridges) emit
        // data bytes with bit 7 set. Spec-wise illegal, but we must
        // not let that smear into the queued pitch/velocity value.
        let cmd = parse(&[0x90, 0xFF, 0xFF]).expect("must parse");
        assert_eq!(cmd.event_type, MidiStatus::NoteOn as u32);
        assert_eq!(cmd.pitch, 0x7F);
        assert_eq!(cmd.velocity, 0x7F);

        // Pitch bend with both data bytes high-bit set: must mask
        // rather than letting LSB|MSB-shifted bits collide.
        let cmd = parse(&[0xE0, 0xFF, 0xFF]).expect("must parse");
        assert_eq!(cmd.event_type, MidiStatus::PitchBend as u32);
        assert_eq!(cmd.pitch, (0x7F << 7) | 0x7F);
    }

    #[test]
    fn parse_strips_channel_nibble() {
        // 0x95 = NoteOn channel 5; status nibble 0x90 should match.
        let cmd = parse(&[0x95, 60, 80]).expect("channel 5 NoteOn");
        assert_eq!(cmd.event_type, MidiStatus::NoteOn as u32);
    }
}
