/// Audio node base class and interface.
/// Every DSP node (oscillator, filter, gain) inherits from this.
///
/// Key design:
/// - Pure virtual process() method for sample processing
/// - Block-based processing (not per-sample) for efficiency
/// - Zero allocations during audio callback
/// - Parameter updates are applied atomically at block boundaries

#pragma once

#include <cstdint>
#include <cstring>
#include "audio_engine.h"

constexpr uint32_t MAX_AUDIO_INPUTS = 4;
constexpr uint32_t MAX_AUDIO_OUTPUTS = 4;
constexpr uint32_t MAX_BLOCK_SIZE = 1024;
constexpr float TWO_PI = 6.283185307f;

/// MIDI event command (must match Rust definition)
struct alignas(16) MIDIEventCmd
{
    uint32_t event_type;
    uint32_t pitch;
    uint32_t velocity;
    uint32_t timestamp_samples;
};

/// Base class for all audio nodes
class AudioNode
{
public:
    uint32_t node_id = 0;
    uint32_t num_inputs = 0;
    uint32_t num_outputs = 0;
    float sample_rate = 48000.0f;

    AudioNode() = default;
    virtual ~AudioNode() = default;

    /// Process a block of audio samples.
    ///
    /// # Arguments:
    /// - inputs: Array of input pointers (or nullptr if no input wired)
    /// - outputs: Array of output buffers to write to
    /// - num_samples: Number of samples in this block (typically 256-512)
    /// - pending_params: Parameter update commands to apply to this node
    /// - num_params: Number of pending parameter updates
    ///
    /// The node should apply pending parameters atomically (within the block)
    /// and process the samples into the output buffers.
    virtual void process(
        const float *const *inputs,
        float **outputs,
        uint32_t num_samples,
        const ParamUpdateCmd *pending_params,
        uint32_t num_params) = 0;

    /// Set a parameter by hash.
    /// This is called from apply_pending_params() to update the node's state.
    virtual void set_param(uint32_t param_hash, float value) {}

    /// Reset internal state (phase, filter history, etc.)
    virtual void reset() {}

    /// Apply pending parameter updates to this node
    void apply_pending_params(const ParamUpdateCmd *pending, uint32_t count)
    {
        for (uint32_t i = 0; i < count; ++i)
        {
            if (pending[i].node_id == node_id)
            {
                set_param(pending[i].param_hash, pending[i].value);
            }
        }
    }

    /// Clear all output buffers
    static void clear_outputs(float **outputs, uint32_t num_outputs, uint32_t num_samples)
    {
        for (uint32_t i = 0; i < num_outputs; ++i)
        {
            std::memset(outputs[i], 0, num_samples * sizeof(float));
        }
    }

    /// Add a scaled input to an output (for mixing)
    static void add_scaled(
        float *dest,
        const float *src,
        float scale,
        uint32_t num_samples)
    {
        for (uint32_t i = 0; i < num_samples; ++i)
        {
            dest[i] += src[i] * scale;
        }
    }

    /// Copy input to output
    static void copy(
        float *dest,
        const float *src,
        uint32_t num_samples)
    {
        std::memcpy(dest, src, num_samples * sizeof(float));
    }
};

/// Parameter hash constants (FNV-1a 32-bit)
namespace ParamHash
{
    // Oscillator
    constexpr uint32_t OSC_FREQUENCY = 2166136261u ^ ('f' ^ 'r' ^ 'e' ^ 'q');
    constexpr uint32_t OSC_AMPLITUDE = 2166136261u ^ ('a' ^ 'm' ^ 'p');

    // Filter
    constexpr uint32_t FILTER_CUTOFF = 2166136261u ^ ('c' ^ 'u' ^ 't');
    constexpr uint32_t FILTER_RESONANCE = 2166136261u ^ ('r' ^ 'e' ^ 's');

    // Gain
    constexpr uint32_t GAIN_LEVEL = 2166136261u ^ ('g' ^ 'a' ^ 'i' ^ 'n');
}
