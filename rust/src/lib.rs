//! Joduga Audio Engine — Rust middleware layer
//!
//! 1. Graph validation and topological sorting  (`shadow_graph`)
//! 2. FFI bridge to the C++ real-time DSP engine  (`ffi`)
//! 3. Lock-free SPSC queues for parameter & MIDI updates  (`lockfree_queue`)
//! 4. Safe wrapper with cpal audio output  (`audio_engine_wrapper`)
//! 5. MIDI input handling  (`midi_input`)

/// Crate version (embedded from Cargo.toml at compile time).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod audio_engine_wrapper;
pub mod ffi;
pub mod lockfree_queue;
pub mod midi_input;
pub mod shadow_graph;

pub use audio_engine_wrapper::{AudioEngineWrapper, OutputRingBuffer};
pub use ffi::{AudioEngine, AudioEngineConfig, CompiledGraph, NodeConnection, NodeDesc, NodeType};
pub use lockfree_queue::{LockFreeRingBuffer, MIDIEventCmd, ParamUpdateCmd, StatusRegister};
pub use midi_input::MidiInputHandler;
pub use shadow_graph::{Edge, Node, ShadowGraph, MAX_EDGES, MAX_NODES};
