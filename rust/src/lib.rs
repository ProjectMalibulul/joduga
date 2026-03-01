pub mod audio_engine_wrapper;
pub mod ffi;
/// Joduga Audio Engine - Rust middleware layer
///
/// This layer is responsible for:
/// 1. Graph validation and topological sorting
/// 2. Communicating with the C++ DSP engine via FFI
/// 3. Managing lock-free queues for real-time parameter updates
/// 4. Handling MIDI input
/// 5. Interfacing with Tauri for the React frontend
pub mod lockfree_queue;
pub mod midi_input;
pub mod shadow_graph;

pub use audio_engine_wrapper::AudioEngineWrapper;
pub use ffi::{AudioEngine, AudioEngineConfig, CompiledGraph, NodeConnection, NodeDesc, NodeType};
pub use lockfree_queue::{LockFreeRingBuffer, MIDIEventCmd, ParamUpdateCmd, StatusRegister};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lockfree_queue_basics() {
        let queue = LockFreeRingBuffer::<u32>::new(16);
        queue.enqueue(42).unwrap();
        let mut out = [0u32; 1];
        let n = queue.dequeue(&mut out);
        assert_eq!(n, 1);
        assert_eq!(out[0], 42);
    }
}
