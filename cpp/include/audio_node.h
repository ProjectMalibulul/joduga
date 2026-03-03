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

/// Parameter hash constants — must match Rust H_FREQ / H_RES exactly.
/// The primary hash (FREQ) is reused across node types since dispatch is per-node.
namespace ParamHash
{
    // ── Primary shared hashes (match Rust constants) ────────────────
    constexpr uint32_t FREQ = 0x811C9DC5u; // H_FREQ — freq / cutoff / level / threshold
    constexpr uint32_t RES = 0x050C5D2Eu;  // H_RES  — resonance / Q / bandwidth

    // Per-type aliases
    constexpr uint32_t OSC_FREQUENCY = FREQ;
    constexpr uint32_t FILTER_CUTOFF = FREQ;
    constexpr uint32_t FILTER_RESONANCE = RES;
    constexpr uint32_t GAIN_LEVEL = FREQ;

    // ── Sub-type selectors ──────────────────────────────────────────
    constexpr uint32_t WAVEFORM_TYPE = 0x000000ADu; // oscillator waveform
    constexpr uint32_t FILTER_MODE = 0x000000BDu;   // filter type (LP/HP/BP/…)

    // ── Oscillator extra params ─────────────────────────────────────
    constexpr uint32_t DUTY_CYCLE = 0xA1u;
    constexpr uint32_t FM_MOD_DEPTH = 0xA3u;
    constexpr uint32_t FM_MOD_FREQ = 0xA4u;
    constexpr uint32_t AM_MOD_DEPTH = 0xA5u;
    constexpr uint32_t AM_MOD_FREQ = 0xA6u;
    constexpr uint32_t DETUNE = 0xA9u;

    // ── Filter extra params ─────────────────────────────────────────
    constexpr uint32_t COMB_DELAY = 0xB1u;
    constexpr uint32_t COMB_FEEDBACK = 0xB2u;
    constexpr uint32_t PARAMETRIC_Q = 0xB7u;

    // ── Dynamics extra params ───────────────────────────────────────
    constexpr uint32_t THRESHOLD = 0xC0u;
    constexpr uint32_t RATIO = 0xC1u;
    constexpr uint32_t ATTACK = 0xC2u;
    constexpr uint32_t RELEASE = 0xC3u;
    constexpr uint32_t DYN_KNEE = 0xC4u;
    constexpr uint32_t DYN_MAKEUP = 0xC5u;

    // ── Sub-type selectors (Gain/Delay/Effects) ─────────────────────
    constexpr uint32_t GAIN_MODE = 0xCFu;
    constexpr uint32_t DELAY_MODE = 0xCDu;
    constexpr uint32_t EFFECTS_MODE = 0xCEu;

    // ── Effects extra params ────────────────────────────────────────
    constexpr uint32_t DELAY_TIME = 0xD1u;
    constexpr uint32_t DELAY_FEEDBACK = 0xD2u;
    constexpr uint32_t MIX = 0xD3u;
    constexpr uint32_t DRIVE = 0xE1u;
    constexpr uint32_t TONE = 0xE2u;

    // ── Modulator extra params ──────────────────────────────────────
    constexpr uint32_t DEPTH = 0x10u;
}
