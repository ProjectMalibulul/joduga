/// Core audio engine implementation.
/// Real-time audio thread with lock-free parameter passing.

#include "audio_engine.h"
#include "audio_node.h"
#include "nodes/oscillator.h"
#include "nodes/filter.h"
#include "nodes/gain.h"
#include "nodes/delay.h"
#include "nodes/effects.h"
#include "platform/rt_platform.h"

#include <thread>
#include <atomic>
#include <memory>
#include <cstring>
#include <ctime>
#include <iostream>
#include <vector>
#include <unordered_map>
#include <algorithm>
#include <chrono>

// ── Internal engine state ──────────────────────────────────────────────
struct AudioEngineImpl
{
    uint32_t sample_rate = 48000;
    uint32_t block_size = 256;
    uint32_t cpu_core = 0;

    std::atomic<bool> is_running{false};
    std::atomic<uint64_t> sample_count{0};
    std::thread audio_thread;

    // Graph
    std::vector<std::unique_ptr<AudioNode>> nodes;
    std::unordered_map<uint32_t, size_t> node_id_to_slot; // node_id → vector index
    std::vector<uint32_t> execution_order;                // stores node_ids
    uint32_t output_node_id = 0;

    // Inter-node scratch buffers (indexed by slot)
    std::vector<std::vector<float>> scratch_buffers;

    // Pre-built wiring  (slot indices, not node IDs)
    struct SlotConn
    {
        uint32_t from_slot;
        uint32_t to_slot;
        uint32_t to_input;
    };
    std::vector<SlotConn> slot_connections;
    // Which slot is the output node (its scratch buffer is copied to ring)
    int32_t output_feeder_slot = -1;

    // Lock-free queues (Rust-owned memory)
    const void *param_queue_buffer = nullptr;
    uint32_t param_queue_capacity = 0;
    const std::atomic<size_t> *param_queue_head = nullptr;
    std::atomic<size_t> *param_queue_tail = nullptr;

    const void *midi_queue_buffer = nullptr;
    uint32_t midi_queue_capacity = 0;
    const std::atomic<size_t> *midi_queue_head = nullptr;
    std::atomic<size_t> *midi_queue_tail = nullptr;

    StatusRegister *status_register = nullptr;

    // Output ring (Rust-owned)
    float *output_ring_buffer = nullptr;
    uint32_t output_ring_capacity = 0;
    std::atomic<size_t> *output_ring_head = nullptr;
    const std::atomic<size_t> *output_ring_tail = nullptr;

    // Working buffer — sized to match the param queue so we never truncate
    std::vector<ParamUpdateCmd> pending_params;
};

// Note: removed global singleton; each engine is now independent.

// ── Node factory ───────────────────────────────────────────────────────
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
    case NODE_TYPE_OUTPUT:
        node = std::make_unique<GainNode>();
        break;
    case NODE_TYPE_DELAY:
        node = std::make_unique<DelayNode>();
        break;
    case NODE_TYPE_EFFECTS:
        node = std::make_unique<EffectsNode>();
        break;
    default:
        std::cerr << "[joduga] Unknown node type: " << type << "\n";
        return nullptr;
    }
    node->node_id = node_id;
    return node;
}

// ── Audio thread ───────────────────────────────────────────────────────
static void audio_thread_main(AudioEngineImpl *e)
{
    rt_platform::set_thread_rt_priority(e->cpu_core);

    // Size the working buffer to match the queue capacity so we never
    // silently drop parameter updates.
    e->pending_params.resize(e->param_queue_capacity);

    // Pre-compute block duration for deadline-based pacing.
    // Instead of sleeping a fixed block_ns after processing (which causes
    // total cycle = processing + block_ns > real-time rate, leading to
    // periodic silence / underruns), we use a deadline approach:
    //   next_deadline = now + block_ns
    //   process block
    //   sleep(next_deadline - now)
    const uint64_t block_ns =
        static_cast<uint64_t>(e->block_size) * 1000000000ULL / e->sample_rate;

    auto get_now_ns = []() -> uint64_t {
        auto now = std::chrono::steady_clock::now();
        return std::chrono::duration_cast<std::chrono::nanoseconds>(now.time_since_epoch()).count();
    };

    // Get initial time reference
    uint64_t next_deadline_ns = get_now_ns() + block_ns;

    while (e->is_running.load(std::memory_order_acquire))
    {
        // ── Drain param queue ────────────────────────────────────
        uint32_t num_params = 0;
        {
            size_t tail = e->param_queue_tail->load(std::memory_order_acquire);
            size_t head = e->param_queue_head->load(std::memory_order_acquire);
            uint32_t avail = (head >= tail)
                                 ? static_cast<uint32_t>(head - tail)
                                 : static_cast<uint32_t>(e->param_queue_capacity - tail + head);

            if (avail > 0 && e->param_queue_buffer)
            {
                const auto *q = static_cast<const ParamUpdateCmd *>(e->param_queue_buffer);
                uint32_t cap = e->param_queue_capacity;
                for (uint32_t i = 0; i < avail && num_params < e->pending_params.size(); ++i)
                    e->pending_params[num_params++] = q[(tail + i) & (cap - 1)];
                e->param_queue_tail->store(
                    (tail + avail) & (e->param_queue_capacity - 1),
                    std::memory_order_release);
            }
        }

        // ── Process graph ────────────────────────────────────────
        for (uint32_t nid : e->execution_order)
        {
            auto it = e->node_id_to_slot.find(nid);
            if (it == e->node_id_to_slot.end())
                continue;
            size_t slot = it->second;
            AudioNode *node = e->nodes[slot].get();

            const float *inputs[MAX_AUDIO_INPUTS] = {};
            for (const auto &c : e->slot_connections)
            {
                if (c.to_slot == slot && c.to_input < MAX_AUDIO_INPUTS)
                    inputs[c.to_input] = e->scratch_buffers[c.from_slot].data();
            }

            float *outputs[MAX_AUDIO_OUTPUTS] = {};
            for (uint32_t i = 0; i < node->num_outputs; ++i)
                outputs[i] = e->scratch_buffers[slot].data();

            node->process(inputs, outputs, e->block_size,
                          e->pending_params.data(), num_params);
        }

        e->sample_count.fetch_add(e->block_size, std::memory_order_release);

        // ── Copy output to ring buffer ───────────────────────────
        if (e->output_ring_buffer && e->output_feeder_slot >= 0)
        {
            size_t oh = e->output_ring_head->load(std::memory_order_acquire);
            size_t ot = e->output_ring_tail->load(std::memory_order_acquire);
            uint32_t cap = e->output_ring_capacity;
            size_t used = (oh >= ot) ? (oh - ot) : (cap - ot + oh);
            size_t free = cap - used - 1;
            uint32_t to_write = std::min(e->block_size, static_cast<uint32_t>(free));

            const float *src = e->scratch_buffers[e->output_feeder_slot].data();
            for (uint32_t i = 0; i < to_write; ++i)
                e->output_ring_buffer[(oh + i) & (cap - 1)] = src[i];

            e->output_ring_head->store((oh + to_write) & (cap - 1),
                                       std::memory_order_release);
        }

        if (e->status_register)
            reinterpret_cast<std::atomic<uint32_t>*>(&e->status_register->graph_version)->fetch_add(1u, std::memory_order_release);

        // ── Deadline-based pacing ────────────────────────────────
        // Sleep only the remaining time until the next block deadline.
        // If we're behind schedule (processing took longer than block_ns),
        // skip the sleep and let the engine catch up.
        uint64_t now_ns = get_now_ns();
        if (now_ns < next_deadline_ns)
        {
            rt_platform::sleep_precise_ns(next_deadline_ns - now_ns);
        }
        next_deadline_ns += block_ns;
    }
}

// ── extern "C" API ─────────────────────────────────────────────────────
extern "C"
{

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
        const void *output_ring_tail)
    {
        if (!graph || !config)
        {
            std::cerr << "[joduga] null graph or config\n";
            return nullptr;
        }

        auto e = std::make_unique<AudioEngineImpl>();
        e->sample_rate = config->sample_rate;
        e->block_size = config->block_size;
        e->cpu_core = config->cpu_core;

        // Create nodes and build id→slot map
        for (uint32_t i = 0; i < graph->num_nodes; ++i)
        {
            const auto &nd = graph->nodes[i];
            auto node = create_node(nd.node_type, nd.node_id);
            if (!node)
                return nullptr;
            node->sample_rate = static_cast<float>(e->sample_rate);
            size_t slot = e->nodes.size();
            e->node_id_to_slot[nd.node_id] = slot;
            e->nodes.push_back(std::move(node));
        }

        // Copy execution order (stores node IDs)
        e->execution_order.assign(graph->execution_order,
                                  graph->execution_order + graph->num_in_order);
        e->output_node_id = graph->output_node_id;

        // Pre-build slot-based connections for O(1) lookup in the audio thread
        for (uint32_t i = 0; i < graph->num_connections; ++i)
        {
            const auto &c = graph->connections[i];
            auto fit = e->node_id_to_slot.find(c.from_node_id);
            auto tit = e->node_id_to_slot.find(c.to_node_id);
            if (fit == e->node_id_to_slot.end() || tit == e->node_id_to_slot.end())
                continue;
            e->slot_connections.push_back({static_cast<uint32_t>(fit->second),
                                           static_cast<uint32_t>(tit->second),
                                           c.to_input_idx});
        }

        // Cache the output node's own slot so we copy its processed scratch
        // buffer to the ring (not the raw feeder).
        {
            auto oit = e->node_id_to_slot.find(e->output_node_id);
            if (oit != e->node_id_to_slot.end())
                e->output_feeder_slot = static_cast<int32_t>(oit->second);
        }

        // Allocate scratch buffers
        e->scratch_buffers.resize(e->nodes.size());
        for (auto &buf : e->scratch_buffers)
            buf.resize(e->block_size, 0.0f);

        // Store queue pointers
        e->param_queue_buffer = param_queue_buffer;
        e->param_queue_capacity = param_queue_capacity;
        e->param_queue_head = static_cast<const std::atomic<size_t> *>(param_queue_head);
        e->param_queue_tail = static_cast<std::atomic<size_t> *>(param_queue_tail);

        e->midi_queue_buffer = midi_queue_buffer;
        e->midi_queue_capacity = midi_queue_capacity;
        e->midi_queue_head = static_cast<const std::atomic<size_t> *>(midi_queue_head);
        e->midi_queue_tail = static_cast<std::atomic<size_t> *>(midi_queue_tail);

        e->status_register = status_register;

        e->output_ring_buffer = output_ring_buffer;
        e->output_ring_capacity = output_ring_capacity;
        e->output_ring_head = output_ring_head
                                  ? static_cast<std::atomic<size_t> *>(output_ring_head)
                                  : nullptr;
        e->output_ring_tail = output_ring_tail
                                  ? static_cast<const std::atomic<size_t> *>(output_ring_tail)
                                  : nullptr;

        auto *raw = e.release();
        return reinterpret_cast<AudioEngine *>(raw);
    }

    int audio_engine_start(AudioEngine *engine_opaque)
    {
        auto *e = reinterpret_cast<AudioEngineImpl *>(engine_opaque);
        if (!e)
            return -1;
        e->is_running.store(true, std::memory_order_release);
        e->audio_thread = std::thread(audio_thread_main, e);
        return 0;
    }

    int audio_engine_stop(AudioEngine *engine_opaque)
    {
        auto *e = reinterpret_cast<AudioEngineImpl *>(engine_opaque);
        if (!e)
            return -1;
        e->is_running.store(false, std::memory_order_release);
        if (e->audio_thread.joinable())
            e->audio_thread.join();
        return 0;
    }

    void audio_engine_destroy(AudioEngine *engine_opaque)
    {
        auto *e = reinterpret_cast<AudioEngineImpl *>(engine_opaque);
        if (!e)
            return;
        if (e->is_running.load(std::memory_order_acquire))
            audio_engine_stop(engine_opaque);
        delete e;
    }

    uint64_t audio_engine_get_sample_count(const AudioEngine *engine_opaque)
    {
        auto *e = reinterpret_cast<const AudioEngineImpl *>(engine_opaque);
        return e ? e->sample_count.load(std::memory_order_acquire) : 0;
    }

    uint8_t audio_engine_is_running(const AudioEngine *engine_opaque)
    {
        auto *e = reinterpret_cast<const AudioEngineImpl *>(engine_opaque);
        return (e && e->is_running.load(std::memory_order_acquire)) ? 1 : 0;
    }

} // extern "C"
