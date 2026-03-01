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

    /// Status register shared between Rust and C++
    typedef struct
    {
        uint32_t graph_version;
        uint32_t adopted_version;
        uint32_t reserved[2];
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
        const void *param_queue_tail,
        const void *midi_queue_buffer,
        uint32_t midi_queue_capacity,
        const void *midi_queue_head,
        const void *midi_queue_tail,
        StatusRegister *status_register);

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
