/// Core audio engine implementation.
/// Real-time audio thread with lock-free parameter passing.

#include "audio_engine.h"
#include "audio_node.h"
#include "nodes/oscillator.h"
#include "nodes/filter.h"
#include "nodes/gain.h"
#include "nodes/delay.h"
#include "nodes/effects.h"
#include "nodes/reverb.h"
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

    // Inter-node scratch buffers, one per node *output* (not per node).
    // Indexed by `output_buffer_offset[slot] + output_idx` so multi-output
    // nodes get distinct buffers for each output port.
    std::vector<std::vector<float>> scratch_buffers;
    std::vector<uint32_t> output_buffer_offset; // size == nodes.size()

    // Pre-built wiring  (slot indices, not node IDs)
    struct SlotConn
    {
        uint32_t from_slot;
        uint32_t from_output;
        uint32_t to_slot;
        uint32_t to_input;
    };
    std::vector<SlotConn> slot_connections;
    // Index into scratch_buffers that the output node fills (its output 0).
    int32_t output_feeder_buffer = -1;

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
    case NODE_TYPE_REVERB:
        node = std::make_unique<ReverbNode>();
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

    auto get_now_ns = []() -> uint64_t
    {
        auto now = std::chrono::steady_clock::now();
        return std::chrono::duration_cast<std::chrono::nanoseconds>(now.time_since_epoch()).count();
    };

    // Get initial time reference
    uint64_t next_deadline_ns = get_now_ns() + block_ns;

    while (e->is_running.load(std::memory_order_acquire))
    {
        const uint64_t loop_start_ns = get_now_ns();

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
                {
                    uint32_t buf_idx = e->output_buffer_offset[c.from_slot] + c.from_output;
                    inputs[c.to_input] = e->scratch_buffers[buf_idx].data();
                }
            }

            float *outputs[MAX_AUDIO_OUTPUTS] = {};
            uint32_t out_base = e->output_buffer_offset[slot];
            for (uint32_t i = 0; i < node->num_outputs; ++i)
                outputs[i] = e->scratch_buffers[out_base + i].data();

            node->process(inputs, outputs, e->block_size,
                          e->pending_params.data(), num_params);
        }

        e->sample_count.fetch_add(e->block_size, std::memory_order_release);

        // ── Copy output to ring buffer ───────────────────────────
        if (e->output_ring_buffer && e->output_feeder_buffer >= 0)
        {
            size_t oh = e->output_ring_head->load(std::memory_order_acquire);
            size_t ot = e->output_ring_tail->load(std::memory_order_acquire);
            uint32_t cap = e->output_ring_capacity;
            size_t used = (oh >= ot) ? (oh - ot) : (cap - ot + oh);
            size_t free = cap - used - 1;
            uint32_t to_write = std::min(e->block_size, static_cast<uint32_t>(free));

            const float *src = e->scratch_buffers[e->output_feeder_buffer].data();
            for (uint32_t i = 0; i < to_write; ++i)
                e->output_ring_buffer[(oh + i) & (cap - 1)] = src[i];

            e->output_ring_head->store((oh + to_write) & (cap - 1),
                                       std::memory_order_release);
        }

        if (e->status_register)
        {
            std::atomic_ref<uint32_t> graph_version_ref(e->status_register->graph_version);
            graph_version_ref.fetch_add(1u, std::memory_order_release);

            const uint64_t proc_ns = get_now_ns() - loop_start_ns;
            const uint32_t load_permil = static_cast<uint32_t>(std::min<uint64_t>(
                4000u,
                (proc_ns * 1000u) / (block_ns ? block_ns : 1u)));
            std::atomic_ref<uint32_t> cpu_load_ref(e->status_register->cpu_load_permil);
            cpu_load_ref.store(load_permil, std::memory_order_release);
        }

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

        // Defensive sanity checks on the FFI inputs.  The Rust side
        // (ShadowGraph::validate) already enforces these, but the engine
        // is also exposed as a plain C ABI and may be linked against
        // other hosts in the future; a malformed graph silently producing
        // no audio (or, worse, dereferencing a null nodes pointer) is a
        // priority-1 failure mode.
        if (config->block_size == 0)
        {
            std::cerr << "[joduga] config->block_size is zero\n";
            return nullptr;
        }
        if (graph->num_nodes > 0 && graph->nodes == nullptr)
        {
            std::cerr << "[joduga] graph->nodes is null but num_nodes > 0\n";
            return nullptr;
        }
        if (graph->num_in_order > 0 && graph->execution_order == nullptr)
        {
            std::cerr << "[joduga] graph->execution_order is null but num_in_order > 0\n";
            return nullptr;
        }
        if (graph->num_connections > 0 && graph->connections == nullptr)
        {
            std::cerr << "[joduga] graph->connections is null but num_connections > 0\n";
            return nullptr;
        }
        // Lock-free queue index math uses (cap - 1) as a power-of-two mask.
        // A non-power-of-two capacity would silently wrap incorrectly and
        // leak commands.  Reject at boot rather than at runtime.
        auto is_pow2_or_zero = [](uint32_t v) {
            return v == 0 || (v & (v - 1)) == 0;
        };
        if (!is_pow2_or_zero(param_queue_capacity) ||
            !is_pow2_or_zero(midi_queue_capacity) ||
            !is_pow2_or_zero(output_ring_capacity))
        {
            std::cerr << "[joduga] queue capacities must be powers of two\n";
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

        // Reject graphs whose declared output_node_id wasn't created above.
        // Without this check, the engine would still start, the per-block
        // ring-feed lookup at line ~337 would silently fail, and the host
        // would see an audio stream that runs but is permanently silent.
        if (graph->num_nodes > 0 &&
            e->node_id_to_slot.find(e->output_node_id) == e->node_id_to_slot.end())
        {
            std::cerr << "[joduga] output_node_id " << e->output_node_id
                      << " is not present in graph->nodes\n";
            return nullptr;
        }

        // Pre-build slot-based connections for O(1) lookup in the audio thread
        for (uint32_t i = 0; i < graph->num_connections; ++i)
        {
            const auto &c = graph->connections[i];
            auto fit = e->node_id_to_slot.find(c.from_node_id);
            auto tit = e->node_id_to_slot.find(c.to_node_id);
            if (fit == e->node_id_to_slot.end() || tit == e->node_id_to_slot.end())
                continue;
            // Drop edges whose source output index is out of range for the
            // resolved C++ node — guards against descriptor/node disagreement.
            uint32_t from_outs = e->nodes[fit->second]->num_outputs;
            if (c.from_output_idx >= from_outs)
            {
                std::cerr << "[joduga] dropping edge: from_output_idx "
                          << c.from_output_idx << " >= node num_outputs "
                          << from_outs << "\n";
                continue;
            }
            e->slot_connections.push_back({static_cast<uint32_t>(fit->second),
                                           c.from_output_idx,
                                           static_cast<uint32_t>(tit->second),
                                           c.to_input_idx});
        }

        // Build per-output scratch buffer offsets and allocate one buffer per
        // node output. Multi-output nodes used to alias all their outputs to
        // a single per-slot scratch buffer; that silently corrupted audio the
        // moment a node had num_outputs > 1.
        e->output_buffer_offset.resize(e->nodes.size());
        uint32_t total_outputs = 0;
        for (size_t s = 0; s < e->nodes.size(); ++s)
        {
            e->output_buffer_offset[s] = total_outputs;
            total_outputs += e->nodes[s]->num_outputs;
        }
        e->scratch_buffers.resize(total_outputs);
        for (auto &buf : e->scratch_buffers)
            buf.resize(e->block_size, 0.0f);

        // Cache the buffer index of the output node's primary output (idx 0)
        // so the ring-copy step doesn't need to recompute it per block.
        {
            auto oit = e->node_id_to_slot.find(e->output_node_id);
            if (oit != e->node_id_to_slot.end() &&
                e->nodes[oit->second]->num_outputs > 0)
            {
                e->output_feeder_buffer =
                    static_cast<int32_t>(e->output_buffer_offset[oit->second]);
            }
        }

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
