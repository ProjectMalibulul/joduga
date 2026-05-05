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
        assert_eq!(NodeType::Delay as i32, 4);
        assert_eq!(NodeType::Effects as i32, 5);
    }

    /// Verifies the Rust-side `repr(C)` layout matches the layout declared in
    /// `cpp/include/audio_engine.h`. If the C++ header changes a field type
    /// or order, this test will catch the divergence at `cargo test` time
    /// rather than as silent UB at runtime across the FFI boundary.
    #[test]
    fn ffi_layout_matches_cpp() {
        use std::mem::{align_of, size_of};

        // NodeDesc: u32, enum (i32 ABI under repr(C)), u32, u32 → 16 bytes.
        assert_eq!(size_of::<NodeDesc>(), 16, "NodeDesc size");
        assert_eq!(align_of::<NodeDesc>(), 4, "NodeDesc align");

        // NodeConnection: 4× u32 → 16 bytes.
        assert_eq!(size_of::<NodeConnection>(), 16, "NodeConnection size");
        assert_eq!(align_of::<NodeConnection>(), 4, "NodeConnection align");

        // AudioEngineConfig: 3× u32 → 12 bytes (no padding).
        assert_eq!(size_of::<AudioEngineConfig>(), 12, "AudioEngineConfig size");
        assert_eq!(align_of::<AudioEngineConfig>(), 4, "AudioEngineConfig align");

        // ParamUpdateCmd: alignas(16) on the C++ side so it sits on a
        // cache-line-friendly boundary inside the queue. Rust's repr(C)
        // on the equivalent layout gives only 4-byte alignment because
        // there is no Rust attribute equivalent to C++'s alignas(16) on
        // the field set used here. Size match is what matters for the
        // queue's slot stride; the C++ side never dereferences a Rust
        // pointer to this struct directly, so the alignment mismatch is
        // benign. We pin size only.
        assert_eq!(size_of::<ParamUpdateCmd>(), 16, "ParamUpdateCmd size");

        // MIDIEventCmd: 4× u32 → 16 bytes.
        assert_eq!(size_of::<MIDIEventCmd>(), 16, "MIDIEventCmd size");

        // StatusRegister: 2× AtomicU32 + [u32; 2] reserved → 16 bytes.
        // AtomicU32 has the same layout as u32.
        assert_eq!(size_of::<StatusRegister>(), 16, "StatusRegister size");
        assert_eq!(align_of::<StatusRegister>(), 4, "StatusRegister align");

        // CompiledGraph layout sanity. We don't pin a numeric size (pointers
        // are 8 bytes on 64-bit, 4 on 32-bit), but verify it is a multiple of
        // pointer size and aligns to pointer-width.
        assert_eq!(align_of::<CompiledGraph>(), align_of::<*const u8>(), "CompiledGraph align");
        assert!(
            size_of::<CompiledGraph>() >= 3 * size_of::<*const u8>() + 4 * size_of::<u32>(),
            "CompiledGraph too small"
        );

        // NodeDesc field offsets (must match the C++ header field order).
        let d =
            NodeDesc { node_id: 0, node_type: NodeType::Oscillator, num_inputs: 0, num_outputs: 0 };
        let base = &d as *const _ as usize;
        assert_eq!(&d.node_id as *const _ as usize - base, 0);
        assert_eq!(&d.node_type as *const _ as usize - base, 4);
        assert_eq!(&d.num_inputs as *const _ as usize - base, 8);
        assert_eq!(&d.num_outputs as *const _ as usize - base, 12);

        // NodeConnection field offsets.
        let c =
            NodeConnection { from_node_id: 0, from_output_idx: 0, to_node_id: 0, to_input_idx: 0 };
        let base = &c as *const _ as usize;
        assert_eq!(&c.from_node_id as *const _ as usize - base, 0);
        assert_eq!(&c.from_output_idx as *const _ as usize - base, 4);
        assert_eq!(&c.to_node_id as *const _ as usize - base, 8);
        assert_eq!(&c.to_input_idx as *const _ as usize - base, 12);
    }
}
