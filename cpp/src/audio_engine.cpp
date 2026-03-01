/// Core audio engine implementation.
/// This is the heart of the synthesizer—the real-time audio thread.

#include "audio_engine.h"
#include "audio_node.h"
#include "nodes/oscillator.h"
#include "nodes/filter.h"
#include "nodes/gain.h"
#include "platform/rt_platform.h"

#include <thread>
#include <atomic>
#include <memory>
#include <cstring>
#include <iostream>
#include <vector>

/// Internal audio engine implementation
struct AudioEngineImpl
{
    // Configuration
    uint32_t sample_rate = 48000;
    uint32_t block_size = 256;
    uint32_t cpu_core = 0;

    // State
    std::atomic<bool> is_running{false};
    std::atomic<uint64_t> sample_count{0};
    std::thread audio_thread;

    // Graph nodes and execution order
    std::vector<std::unique_ptr<AudioNode>> nodes;
    std::vector<uint32_t> execution_order;
    uint32_t output_node_id = 0;

    // Scratch buffers for inter-node communication
    std::vector<std::vector<float>> scratch_buffers;

    // Connection wiring
    std::vector<NodeConnection> connections;

    // Lock-free queue pointers (Rust side owns the actual buffers)
    const void *param_queue_buffer = nullptr;
    uint32_t param_queue_capacity = 0;
    const std::atomic<size_t> *param_queue_head = nullptr;
    std::atomic<size_t> *param_queue_tail = nullptr;

    const void *midi_queue_buffer = nullptr;
    uint32_t midi_queue_capacity = 0;
    const std::atomic<size_t> *midi_queue_head = nullptr;
    std::atomic<size_t> *midi_queue_tail = nullptr;

    StatusRegister *status_register = nullptr;

    // Working buffers
    std::vector<ParamUpdateCmd> pending_params;
};

/// Global audio engine (accessed from the audio thread)
/// Only one instance is allowed at a time
static AudioEngineImpl *g_audio_engine = nullptr;

/// Create a node based on type
static std::unique_ptr<AudioNode> create_node(NodeType type, uint32_t node_id)
{
    std::unique_ptr<AudioNode> node;
    switch (type)
    {
    case NODE_TYPE_OSCILLATOR:
        node = std::make_unique<OscillatorNode>();
        break;
    case NODE_TYPE_FILTER:
        node = std::make_unique<FilterNode>();
        break;
    case NODE_TYPE_GAIN:
    case NODE_TYPE_OUTPUT: // Treat output as a gain node for now
        node = std::make_unique<GainNode>();
        break;
    default:
        std::cerr << "Unknown node type: " << type << std::endl;
        return nullptr;
    }
    node->node_id = node_id;
    return node;
}

/// Audio processing thread function
static void audio_thread_main(AudioEngineImpl *engine)
{
    // Set real-time priority and CPU affinity
    if (rt_platform::set_thread_rt_priority(engine->cpu_core) != 0)
    {
        std::cerr << "Warning: Could not set real-time priority" << std::endl;
    }

    // Pre-allocate parameter update working buffer
    engine->pending_params.resize(256); // Assume max 256 param updates per block

    // Main audio loop
    while (engine->is_running.load(std::memory_order_acquire))
    {
        // Drain parameter queue and apply updates
        size_t tail = engine->param_queue_tail->load(std::memory_order_acquire);
        size_t head = engine->param_queue_head->load(std::memory_order_acquire);

        // Calculate available commands
        uint32_t available = 0;
        if (head >= tail)
        {
            available = head - tail;
        }
        else
        {
            available = engine->param_queue_capacity - tail + head;
        }

        // Copy pending parameter updates
        uint32_t num_params = 0;
        if (available > 0 && engine->param_queue_buffer != nullptr)
        {
            const ParamUpdateCmd *queue = static_cast<const ParamUpdateCmd *>(engine->param_queue_buffer);
            for (uint32_t i = 0; i < available && num_params < engine->pending_params.size(); ++i)
            {
                uint32_t idx = (tail + i) & (engine->param_queue_capacity - 1);
                engine->pending_params[num_params++] = queue[idx];
            }
            // Update tail
            engine->param_queue_tail->store((tail + available) & (engine->param_queue_capacity - 1), std::memory_order_release);
        }

        // Process each node in topologically-sorted order
        for (uint32_t node_idx : engine->execution_order)
        {
            if (node_idx >= engine->nodes.size())
            {
                std::cerr << "Invalid node index in execution order: " << node_idx << std::endl;
                continue;
            }

            AudioNode *node = engine->nodes[node_idx].get();

            // Gather input pointers from connections
            const float *inputs[MAX_AUDIO_INPUTS] = {nullptr};
            for (const auto& conn : engine->connections) {
                if (conn.to_node_id == node_idx && conn.to_input_idx < MAX_AUDIO_INPUTS) {
                    if (conn.from_node_id < engine->scratch_buffers.size()) {
                        inputs[conn.to_input_idx] = engine->scratch_buffers[conn.from_node_id].data();
                    }
                }
            }

            // Gather output pointers
            float *outputs[MAX_AUDIO_OUTPUTS] = {nullptr};
            for (uint32_t i = 0; i < node->num_outputs; ++i)
            {
                outputs[i] = engine->scratch_buffers[node_idx].data();
            }

            // Process node
            node->process(inputs, outputs, engine->block_size, engine->pending_params.data(), num_params);
        }

        // Increment sample count
        engine->sample_count.fetch_add(engine->block_size, std::memory_order_release);

        // Update status register
        if (engine->status_register != nullptr)
        {
            engine->status_register->graph_version++;
        }

        // TODO: Write output to audio device
    }
}

extern "C"
{

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
        StatusRegister *status_register)
    {
        // Prevent multiple instances
        if (g_audio_engine != nullptr)
        {
            std::cerr << "Audio engine already initialized" << std::endl;
            return nullptr;
        }

        auto engine = std::make_unique<AudioEngineImpl>();

        // Copy configuration
        engine->sample_rate = config->sample_rate;
        engine->block_size = config->block_size;
        engine->cpu_core = config->cpu_core;

        // Create nodes from graph
        for (uint32_t i = 0; i < graph->num_nodes; ++i)
        {
            const NodeDesc &node_desc = graph->nodes[i];
            auto node = create_node(node_desc.node_type, node_desc.node_id);
            if (!node)
            {
                std::cerr << "Failed to create node " << i << std::endl;
                return nullptr;
            }
            node->sample_rate = engine->sample_rate;
            engine->nodes.push_back(std::move(node));
        }

        // Copy execution order
        engine->execution_order.resize(graph->num_in_order);
        std::memcpy(engine->execution_order.data(), graph->execution_order, graph->num_in_order * sizeof(uint32_t));
        engine->output_node_id = graph->output_node_id;

        // Copy connections
        engine->connections.resize(graph->num_connections);
        for (uint32_t i = 0; i < graph->num_connections; ++i) {
            engine->connections[i] = graph->connections[i];
        }

        // Allocate scratch buffers for inter-node communication
        engine->scratch_buffers.resize(engine->nodes.size());
        for (auto &buffer : engine->scratch_buffers)
        {
            buffer.resize(engine->block_size);
        }

        // Store queue pointers
        engine->param_queue_buffer = param_queue_buffer;
        engine->param_queue_capacity = param_queue_capacity;
        engine->param_queue_head = static_cast<const std::atomic<size_t> *>(param_queue_head);
        engine->param_queue_tail = const_cast<std::atomic<size_t> *>(static_cast<const std::atomic<size_t> *>(param_queue_tail));

        engine->midi_queue_buffer = midi_queue_buffer;
        engine->midi_queue_capacity = midi_queue_capacity;
        engine->midi_queue_head = static_cast<const std::atomic<size_t> *>(midi_queue_head);
        engine->midi_queue_tail = const_cast<std::atomic<size_t> *>(static_cast<const std::atomic<size_t> *>(midi_queue_tail));

        engine->status_register = status_register;

        g_audio_engine = engine.release();
        return reinterpret_cast<AudioEngine *>(g_audio_engine);
    }

    int audio_engine_start(AudioEngine *engine_opaque)
    {
        auto engine = reinterpret_cast<AudioEngineImpl *>(engine_opaque);
        if (!engine)
        {
            return -1;
        }

        engine->is_running.store(true, std::memory_order_release);
        engine->audio_thread = std::thread(audio_thread_main, engine);

        return 0;
    }

    int audio_engine_stop(AudioEngine *engine_opaque)
    {
        auto engine = reinterpret_cast<AudioEngineImpl *>(engine_opaque);
        if (!engine)
        {
            return -1;
        }

        engine->is_running.store(false, std::memory_order_release);
        if (engine->audio_thread.joinable())
        {
            engine->audio_thread.join();
        }

        return 0;
    }

    void audio_engine_destroy(AudioEngine *engine_opaque)
    {
        auto engine = reinterpret_cast<AudioEngineImpl *>(engine_opaque);
        if (!engine)
        {
            return;
        }

        if (engine->is_running.load(std::memory_order_acquire))
        {
            audio_engine_stop(engine_opaque);
        }

        delete engine;
        g_audio_engine = nullptr;
    }

    uint64_t audio_engine_get_sample_count(const AudioEngine *engine_opaque)
    {
        auto engine = reinterpret_cast<const AudioEngineImpl *>(engine_opaque);
        if (!engine)
        {
            return 0;
        }
        return engine->sample_count.load(std::memory_order_acquire);
    }

    uint8_t audio_engine_is_running(const AudioEngine *engine_opaque)
    {
        auto engine = reinterpret_cast<const AudioEngineImpl *>(engine_opaque);
        if (!engine)
        {
            return 0;
        }
        return engine->is_running.load(std::memory_order_acquire) ? 1 : 0;
    }

} // extern "C"
