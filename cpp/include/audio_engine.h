/// Main audio engine interface.
/// This is the C FFI boundary—all functions use extern "C" and repr(C) structures.
///
/// Lifetime:
/// 1. Rust calls audio_engine_init() with compiled graph and command queues
/// 2. C++ creates AudioEngine, spawns a SCHED_FIFO audio thread
/// 3. C++ audio thread continuously processes audio blocks
/// 4. Rust writes parameter updates to the param queue
/// 5. C++ audio thread drains param queue and applies updates
/// 6. Rust calls audio_engine_stop() to gracefully shut down
/// 7. Rust calls audio_engine_destroy() to free resources

#pragma once

#include <cstdint>
#include <cstddef>

extern "C"
{

    /// Node type enumeration (must match Rust)
    typedef enum
    {
        NODE_TYPE_OSCILLATOR = 0,
        NODE_TYPE_FILTER = 1,
        NODE_TYPE_GAIN = 2,
        NODE_TYPE_OUTPUT = 3,
        NODE_TYPE_DELAY = 4,
        NODE_TYPE_EFFECTS = 5,
        NODE_TYPE_REVERB = 6,
    } NodeType;

    /// Node description (must match Rust CompiledGraph layout)
    typedef struct
    {
        uint32_t node_id;
        NodeType node_type;
        uint32_t num_inputs;
        uint32_t num_outputs;
    } NodeDesc;

    /// Wire connection between nodes
    typedef struct
    {
        uint32_t from_node_id;
        uint32_t from_output_idx;
        uint32_t to_node_id;
        uint32_t to_input_idx;
    } NodeConnection;

    /// Compiled graph sent from Rust
    typedef struct
    {
        const NodeDesc *nodes;
        uint32_t num_nodes;
        const NodeConnection *connections;
        uint32_t num_connections;
        const uint32_t *execution_order;
        uint32_t num_in_order;
        uint32_t output_node_id;
    } CompiledGraph;

    /// Configuration for audio engine
    typedef struct
    {
        uint32_t sample_rate;
        uint32_t block_size;
        uint32_t cpu_core; // CPU core to pin audio thread to
    } AudioEngineConfig;

    /// Parameter update command (must match Rust)
    typedef struct alignas(16)
    {
        uint32_t node_id;
        uint32_t param_hash;
        float value;
        uint32_t padding;
    } ParamUpdateCmd;

    /// Status register shared between Rust and C++.
    /// Fields are plain integers for stable ABI layout across languages.
    /// Each side must access them atomically via atomic_ref / AtomicU32::from_ptr.
    typedef struct
    {
        uint32_t graph_version;
        uint32_t adopted_version;
        uint32_t cpu_load_permil;
        uint32_t reserved;
    } StatusRegister;

    /// Opaque audio engine handle
    typedef struct AudioEngineImpl AudioEngine;

    /// Initialize the audio engine.
    ///
    /// # Arguments:
    /// - graph: Compiled graph from Rust
    /// - config: Configuration (sample rate, block size, etc.)
    /// - param_queue_buffer: Pointer to parameter command buffer
    /// - param_queue_capacity: Capacity of param queue (power of 2)
    /// - param_queue_head: Pointer to head index
    /// - param_queue_tail: Pointer to tail index
    /// - midi_queue_buffer: Pointer to MIDI event buffer
    /// - midi_queue_capacity: Capacity of MIDI queue
    /// - midi_queue_head: Pointer to head index
    /// - midi_queue_tail: Pointer to tail index
    /// - status_register: Pointer to StatusRegister for coordination
    ///
    /// # Returns:
    /// Opaque AudioEngine pointer, or NULL on failure.
    AudioEngine *audio_engine_init(
        const CompiledGraph *graph,
        const AudioEngineConfig *config,
        const void *param_queue_buffer,
        uint32_t param_queue_capacity,
        const void *param_queue_head,
        void *param_queue_tail,
        const void *midi_queue_buffer,
        uint32_t midi_queue_capacity,
        const void *midi_queue_head,
        void *midi_queue_tail,
        StatusRegister *status_register,
        float *output_ring_buffer,
        uint32_t output_ring_capacity,
        void *output_ring_head,
        const void *output_ring_tail);

    /// Start the audio engine (spawns background audio thread).
    /// Returns 0 on success, -1 on failure.
    int audio_engine_start(AudioEngine *engine);

    /// Stop the audio engine gracefully.
    /// Returns 0 on success, -1 on failure.
    int audio_engine_stop(AudioEngine *engine);

    /// Destroy the audio engine and free all resources.
    void audio_engine_destroy(AudioEngine *engine);

    /// Get the current sample count (for MIDI timestamp references).
    uint64_t audio_engine_get_sample_count(const AudioEngine *engine);

    /// Check if the engine is currently running.
    /// Returns 1 if running, 0 if stopped.
    uint8_t audio_engine_is_running(const AudioEngine *engine);

} // extern "C"

// ── ABI layout guards ──────────────────────────────────────────────
//
// Mirror of the offset_of! / size_of / align_of tests in
// rust/src/ffi.rs and rust/src/lockfree_queue.rs. If a field is
// reordered on either side, this fails at C++ compile time (and the
// matching Rust test fails on `cargo test`), so the FFI cannot
// silently desync.
static_assert(sizeof(NodeDesc) == 16, "NodeDesc size drift vs Rust");
static_assert(alignof(NodeDesc) == 4, "NodeDesc alignment drift vs Rust");
static_assert(offsetof(NodeDesc, node_id) == 0, "NodeDesc.node_id offset");
static_assert(offsetof(NodeDesc, node_type) == 4, "NodeDesc.node_type offset");
static_assert(offsetof(NodeDesc, num_inputs) == 8, "NodeDesc.num_inputs offset");
static_assert(offsetof(NodeDesc, num_outputs) == 12, "NodeDesc.num_outputs offset");

static_assert(sizeof(NodeConnection) == 16, "NodeConnection size drift vs Rust");
static_assert(alignof(NodeConnection) == 4, "NodeConnection alignment drift vs Rust");
static_assert(offsetof(NodeConnection, from_node_id) == 0, "NodeConnection.from_node_id offset");
static_assert(offsetof(NodeConnection, from_output_idx) == 4, "NodeConnection.from_output_idx offset");
static_assert(offsetof(NodeConnection, to_node_id) == 8, "NodeConnection.to_node_id offset");
static_assert(offsetof(NodeConnection, to_input_idx) == 12, "NodeConnection.to_input_idx offset");

static_assert(sizeof(AudioEngineConfig) == 12, "AudioEngineConfig size drift vs Rust");
static_assert(alignof(AudioEngineConfig) == 4, "AudioEngineConfig alignment drift vs Rust");
static_assert(offsetof(AudioEngineConfig, sample_rate) == 0, "AudioEngineConfig.sample_rate offset");
static_assert(offsetof(AudioEngineConfig, block_size) == 4, "AudioEngineConfig.block_size offset");
static_assert(offsetof(AudioEngineConfig, cpu_core) == 8, "AudioEngineConfig.cpu_core offset");

static_assert(sizeof(ParamUpdateCmd) == 16, "ParamUpdateCmd size drift vs Rust");
static_assert(alignof(ParamUpdateCmd) == 16, "ParamUpdateCmd alignment drift vs Rust");
static_assert(offsetof(ParamUpdateCmd, node_id) == 0, "ParamUpdateCmd.node_id offset");
static_assert(offsetof(ParamUpdateCmd, param_hash) == 4, "ParamUpdateCmd.param_hash offset");
static_assert(offsetof(ParamUpdateCmd, value) == 8, "ParamUpdateCmd.value offset");
static_assert(offsetof(ParamUpdateCmd, padding) == 12, "ParamUpdateCmd.padding offset");

static_assert(sizeof(StatusRegister) == 16, "StatusRegister size drift vs Rust");
static_assert(alignof(StatusRegister) == 4, "StatusRegister alignment drift vs Rust");
static_assert(offsetof(StatusRegister, graph_version) == 0, "StatusRegister.graph_version offset");
static_assert(offsetof(StatusRegister, adopted_version) == 4, "StatusRegister.adopted_version offset");
static_assert(offsetof(StatusRegister, cpu_load_permil) == 8, "StatusRegister.cpu_load_permil offset");
static_assert(offsetof(StatusRegister, reserved) == 12, "StatusRegister.reserved offset");

#if UINTPTR_MAX == 0xFFFFFFFFFFFFFFFFu
// 64-bit only: pointer width determines CompiledGraph layout.
static_assert(sizeof(CompiledGraph) == 48, "CompiledGraph size drift vs Rust (64-bit)");
static_assert(alignof(CompiledGraph) == 8, "CompiledGraph alignment drift vs Rust (64-bit)");
static_assert(offsetof(CompiledGraph, nodes) == 0, "CompiledGraph.nodes offset");
static_assert(offsetof(CompiledGraph, num_nodes) == 8, "CompiledGraph.num_nodes offset");
static_assert(offsetof(CompiledGraph, connections) == 16, "CompiledGraph.connections offset");
static_assert(offsetof(CompiledGraph, num_connections) == 24, "CompiledGraph.num_connections offset");
static_assert(offsetof(CompiledGraph, execution_order) == 32, "CompiledGraph.execution_order offset");
static_assert(offsetof(CompiledGraph, num_in_order) == 40, "CompiledGraph.num_in_order offset");
static_assert(offsetof(CompiledGraph, output_node_id) == 44, "CompiledGraph.output_node_id offset");
#endif
