/// FFI bindings to the C++ audio engine.
///
/// This module defines the extern "C" interface between Rust and C++.
/// All structures are repr(C) to ensure ABI compatibility.
use libc::{c_float, c_void, uint32_t};

// Re-export queue types for convenience
pub use crate::lockfree_queue::{MIDIEventCmd, ParamUpdateCmd, StatusRegister};

/// Node type enumeration (must match C++ enum)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Oscillator = 0,
    Filter = 1,
    Gain = 2,
    Output = 3,
}

/// Compiled graph representation sent to C++
#[repr(C)]
#[derive(Debug, Clone)]
pub struct CompiledGraph {
    pub nodes: *const NodeDesc,
    pub num_nodes: uint32_t,
    pub connections: *const NodeConnection,
    pub num_connections: uint32_t,
    pub execution_order: *const uint32_t,
    pub num_in_order: uint32_t,
    pub output_node_id: uint32_t,
}

/// Describes a single node in the graph
#[repr(C)]
#[derive(Debug, Clone)]
pub struct NodeDesc {
    pub node_id: uint32_t,
    pub node_type: NodeType,
    pub num_inputs: uint32_t,
    pub num_outputs: uint32_t,
    // Parameters are passed separately via the param update queue
}

/// Describes a wire between two nodes
#[repr(C)]
#[derive(Debug, Clone)]
pub struct NodeConnection {
    pub from_node_id: uint32_t,
    pub from_output_idx: uint32_t,
    pub to_node_id: uint32_t,
    pub to_input_idx: uint32_t,
}

/// Opaque audio engine handle (C++ side)
pub struct AudioEngine {
    _private: [u8; 0],
}

/// Audio engine initialization parameters
#[repr(C)]
#[derive(Debug)]
pub struct AudioEngineConfig {
    pub sample_rate: uint32_t,
    pub block_size: uint32_t,
    pub cpu_core: uint32_t, // CPU core to pin audio thread to
}

extern "C" {
    /// Initialize the audio engine with a compiled graph and command queues.
    ///
    /// # Arguments
    /// - `graph`: Pointer to CompiledGraph structure
    /// - `config`: Configuration parameters
    /// - `param_queue_buffer`: Pointer to parameter command ring buffer
    /// - `param_queue_capacity`: Capacity of param queue (power of 2)
    /// - `param_queue_head`: Pointer to head index (AtomicUsize from Rust)
    /// - `param_queue_tail`: Pointer to tail index (AtomicUsize from Rust)
    /// - `midi_queue_buffer`: Pointer to MIDI event ring buffer
    /// - `midi_queue_capacity`: Capacity of MIDI queue
    /// - `midi_queue_head`: Pointer to head index
    /// - `midi_queue_tail`: Pointer to tail index
    /// - `status_register`: Pointer to StatusRegister
    ///
    /// # Returns
    /// Opaque pointer to AudioEngine, or null on failure.
    pub fn audio_engine_init(
        graph: *const CompiledGraph,
        config: *const AudioEngineConfig,
        param_queue_buffer: *const c_void,
        param_queue_capacity: uint32_t,
        param_queue_head: *const c_void,
        param_queue_tail: *const c_void,
        midi_queue_buffer: *const c_void,
        midi_queue_capacity: uint32_t,
        midi_queue_head: *const c_void,
        midi_queue_tail: *const c_void,
        status_register: *mut StatusRegister,
    ) -> *mut AudioEngine;

    /// Start the audio engine's background thread
    pub fn audio_engine_start(engine: *mut AudioEngine) -> i32;

    /// Stop the audio engine gracefully
    pub fn audio_engine_stop(engine: *mut AudioEngine) -> i32;

    /// Destroy the audio engine and free resources
    pub fn audio_engine_destroy(engine: *mut AudioEngine);

    /// Get the current sample count (for MIDI timestamps)
    pub fn audio_engine_get_sample_count(engine: *const AudioEngine) -> u64;

    /// Check if the audio engine is running
    pub fn audio_engine_is_running(engine: *const AudioEngine) -> u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_type_repr() {
        assert_eq!(NodeType::Oscillator as i32, 0);
        assert_eq!(NodeType::Filter as i32, 1);
        assert_eq!(NodeType::Gain as i32, 2);
        assert_eq!(NodeType::Output as i32, 3);
    }
}
