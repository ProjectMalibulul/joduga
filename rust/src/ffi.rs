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
    Delay = 4,
    Effects = 5,
    Reverb = 6,
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
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn node_type_repr() {
        assert_eq!(NodeType::Oscillator as i32, 0);
        assert_eq!(NodeType::Filter as i32, 1);
        assert_eq!(NodeType::Gain as i32, 2);
        assert_eq!(NodeType::Output as i32, 3);
        assert_eq!(NodeType::Delay as i32, 4);
        assert_eq!(NodeType::Effects as i32, 5);
        assert_eq!(NodeType::Reverb as i32, 6);
    }

    /// Pin NodeDesc layout to the C++ side in cpp/include/audio_engine.h:34-40.
    /// A reorder on either end would silently swap node_id and num_inputs.
    #[test]
    fn node_desc_abi_layout() {
        assert_eq!(size_of::<NodeDesc>(), 16);
        assert_eq!(align_of::<NodeDesc>(), 4);
        assert_eq!(offset_of!(NodeDesc, node_id), 0);
        assert_eq!(offset_of!(NodeDesc, node_type), 4);
        assert_eq!(offset_of!(NodeDesc, num_inputs), 8);
        assert_eq!(offset_of!(NodeDesc, num_outputs), 12);
    }

    /// Pin NodeConnection layout to cpp/include/audio_engine.h:43-49.
    #[test]
    fn node_connection_abi_layout() {
        assert_eq!(size_of::<NodeConnection>(), 16);
        assert_eq!(align_of::<NodeConnection>(), 4);
        assert_eq!(offset_of!(NodeConnection, from_node_id), 0);
        assert_eq!(offset_of!(NodeConnection, from_output_idx), 4);
        assert_eq!(offset_of!(NodeConnection, to_node_id), 8);
        assert_eq!(offset_of!(NodeConnection, to_input_idx), 12);
    }

    /// Pin AudioEngineConfig layout to cpp/include/audio_engine.h:64-69.
    #[test]
    fn audio_engine_config_abi_layout() {
        assert_eq!(size_of::<AudioEngineConfig>(), 12);
        assert_eq!(align_of::<AudioEngineConfig>(), 4);
        assert_eq!(offset_of!(AudioEngineConfig, sample_rate), 0);
        assert_eq!(offset_of!(AudioEngineConfig, block_size), 4);
        assert_eq!(offset_of!(AudioEngineConfig, cpu_core), 8);
    }

    /// Pin CompiledGraph layout to cpp/include/audio_engine.h:53-61. The
    /// pointer size is 64-bit on every platform we currently target; the
    /// 32-bit case is left unchecked because the engine has no 32-bit CI.
    #[cfg(target_pointer_width = "64")]
    #[test]
    fn compiled_graph_abi_layout_64bit() {
        assert_eq!(size_of::<CompiledGraph>(), 48);
        assert_eq!(align_of::<CompiledGraph>(), 8);
        assert_eq!(offset_of!(CompiledGraph, nodes), 0);
        assert_eq!(offset_of!(CompiledGraph, num_nodes), 8);
        // 4 bytes of padding before the next pointer
        assert_eq!(offset_of!(CompiledGraph, connections), 16);
        assert_eq!(offset_of!(CompiledGraph, num_connections), 24);
        // 4 more bytes of padding
        assert_eq!(offset_of!(CompiledGraph, execution_order), 32);
        assert_eq!(offset_of!(CompiledGraph, num_in_order), 40);
        assert_eq!(offset_of!(CompiledGraph, output_node_id), 44);
    }
}
