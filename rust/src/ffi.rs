/// FFI bindings to the C++ audio engine.
///
/// All structures are `#[repr(C)]` so layout matches the C++ side exactly.
///
/// # Safety contracts
/// - All pointers passed to C++ must remain valid for the engine lifetime.
/// - Queue pointers (head/tail) are `AtomicUsize` and must not be freed
///   while the engine is running.
/// - The caller must call `audio_engine_stop` before `audio_engine_destroy`.
use std::ffi::c_void;

pub use crate::lockfree_queue::{MIDIEventCmd, ParamUpdateCmd, StatusRegister};

// ── Enums ───────────────────────────────────────────────────────────────

/// Node type enumeration (must match C++ `NodeType`)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeType {
    Oscillator = 0,
    Filter = 1,
    Gain = 2,
    Output = 3,
}

// ── repr(C) structs ─────────────────────────────────────────────────────

#[repr(C)]
#[derive(Debug, Clone)]
pub struct CompiledGraph {
    pub nodes: *const NodeDesc,
    pub num_nodes: u32,
    pub connections: *const NodeConnection,
    pub num_connections: u32,
    pub execution_order: *const u32,
    pub num_in_order: u32,
    pub output_node_id: u32,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct NodeDesc {
    pub node_id: u32,
    pub node_type: NodeType,
    pub num_inputs: u32,
    pub num_outputs: u32,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct NodeConnection {
    pub from_node_id: u32,
    pub from_output_idx: u32,
    pub to_node_id: u32,
    pub to_input_idx: u32,
}

/// Opaque handle returned by `audio_engine_init`.
#[repr(C)]
pub struct AudioEngine {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Debug)]
pub struct AudioEngineConfig {
    pub sample_rate: u32,
    pub block_size: u32,
    pub cpu_core: u32,
}

// ── extern "C" ──────────────────────────────────────────────────────────

extern "C" {
    pub fn audio_engine_init(
        graph: *const CompiledGraph,
        config: *const AudioEngineConfig,
        param_queue_buffer: *const c_void,
        param_queue_capacity: u32,
        param_queue_head: *const c_void,
        param_queue_tail: *mut c_void, // consumer (C++) writes tail
        midi_queue_buffer: *const c_void,
        midi_queue_capacity: u32,
        midi_queue_head: *const c_void,
        midi_queue_tail: *mut c_void, // consumer (C++) writes tail
        status_register: *mut StatusRegister,
        output_ring_buffer: *mut f32,
        output_ring_capacity: u32,
        output_ring_head: *mut c_void,
        output_ring_tail: *const c_void,
    ) -> *mut AudioEngine;

    pub fn audio_engine_start(engine: *mut AudioEngine) -> i32;
    pub fn audio_engine_stop(engine: *mut AudioEngine) -> i32;
    pub fn audio_engine_destroy(engine: *mut AudioEngine);
    pub fn audio_engine_get_sample_count(engine: *const AudioEngine) -> u64;
    pub fn audio_engine_is_running(engine: *const AudioEngine) -> u8;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_type_repr() {
        assert_eq!(NodeType::Oscillator as i32, 0);
        assert_eq!(NodeType::Filter as i32, 1);
        assert_eq!(NodeType::Gain as i32, 2);
        assert_eq!(NodeType::Output as i32, 3);
    }
}
