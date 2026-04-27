/// Effects processing node.
/// Supports: distortion, overdrive, bitcrusher, ring modulator,
///           waveshaper, tremolo, stereo widener (mono output).
///
/// The effect mode is selected via the EFFECTS_MODE parameter (set at init).

#pragma once

#include "audio_node.h"
#include <algorithm>
#include <cmath>
#include <cstring>

class EffectsNode : public AudioNode
{
public:
    enum Mode
    {
        DISTORTION = 0,
        OVERDRIVE = 1,
        BITCRUSHER = 2,
        RING_MOD = 3,
        WAVESHAPER = 4,
        TREMOLO = 5,
        STEREO_WIDENER = 6,
    };

private:
    int mode = DISTORTION;

    // Distortion / Overdrive
    float drive = 1.0f;
    float tone = 0.5f;
    float distort_mix = 1.0f;
    float tone_lp = 0.0f;

    // Bitcrusher
    float bit_depth = 8.0f;
    float sample_rate_reduce = 1.0f; // 1 = no reduction, higher = more crushed
    float crush_held = 0.0f;
    float crush_counter = 0.0f;

    // Ring Modulator
    float ring_freq = 440.0f;
    float ring_mix = 1.0f;
    float ring_phase = 0.0f;

    // Waveshaper
    float ws_amount = 1.0f;
    float ws_mix = 1.0f;

    // Tremolo
    float trem_rate = 4.0f;
    float trem_depth = 0.5f;
    float trem_phase = 0.0f;

    // Stereo widener (mono approx - allpass decorrelation)
    float width = 0.5f;
    static constexpr int AP_LEN = 512;
    float ap_buf[AP_LEN] = {};
    int ap_pos = 0;

public:
    EffectsNode() : AudioNode()
    {
        num_inputs = 1;
        num_outputs = 1;
    }
    virtual ~EffectsNode() = default;

    void set_param(uint32_t param_hash, float value) override
    {
        if (!std::isfinite(value))
            return;
        switch (param_hash)
        {
        case 0x000000CEu: // EFFECTS_MODE
        {
            // Guard against `static_cast<int>(NaN)` UB (already
            // handled by the early-return above) and clamp to the
            // declared enum range so an out-of-range cast can't
            // drive process() into the silent default arm.
            int m = static_cast<int>(value);
            if (m < 0)
                m = 0;
            else if (m > STEREO_WIDENER)
                m = STEREO_WIDENER;
            mode = m;
            break;
        }

        // ── Distortion (catalog hashes 0xE1, 0xE2, 0xE3) ──
        case 0xE1u: // Distortion Drive
            drive = std::fmax(0.1f, std::fmin(value, 100.0f));
            break;
        case 0xE2u: // Distortion Tone
            tone = std::fmax(0.0f, std::fmin(value, 1.0f));
            break;
        case 0xE3u: // Distortion Mix
            distort_mix = std::fmax(0.0f, std::fmin(value, 1.0f));
            break;

        // ── Overdrive (catalog hashes 0xE4, 0xE5) ──
        case 0xE4u: // Overdrive Drive
            drive = std::fmax(0.1f, std::fmin(value, 100.0f));
            break;
        case 0xE5u: // Overdrive Tone
            tone = std::fmax(0.0f, std::fmin(value, 1.0f));
            break;

        // ── Bitcrusher (catalog hashes 0xE6, 0xE7) ──
        case 0xE6u: // Bit depth
            bit_depth = std::fmax(1.0f, std::fmin(value, 16.0f));
            break;
        case 0xE7u: // Rate reduction
            sample_rate_reduce = std::fmax(1.0f, std::fmin(value, 100.0f));
            break;

        // ── Ring Modulator (catalog hashes 0xE8, 0xE9) ──
        case 0xE8u: // Ring freq
            ring_freq = std::fmax(1.0f, std::fmin(value, 20000.0f));
            break;
        case 0xE9u: // Ring mix
            ring_mix = std::fmax(0.0f, std::fmin(value, 1.0f));
            break;

        // ── Tremolo (catalog hashes 0xEA, 0xEB) ──
        case 0xEAu: // Tremolo rate
            trem_rate = std::fmax(0.1f, std::fmin(value, 50.0f));
            break;
        case 0xEBu: // Tremolo depth
            trem_depth = std::fmax(0.0f, std::fmin(value, 1.0f));
            break;

        // ── Waveshaper (catalog hashes 0xEE, 0xEF) ──
        case 0xEEu: // Waveshaper amount
            ws_amount = std::fmax(0.1f, std::fmin(value, 50.0f));
            break;
        case 0xEFu: // Waveshaper symmetry → mix
            ws_mix = std::fmax(0.0f, std::fmin(value, 1.0f));
            break;

        // ── Stereo Widener (catalog hash 0xF2) ──
        case 0xF2u: // Width
            width = std::fmax(0.0f, std::fmin(value, 2.0f));
            break;
        }
    }

    void reset() override
    {
        tone_lp = 0.0f;
        crush_held = 0.0f;
        crush_counter = 0.0f;
        ring_phase = 0.0f;
        trem_phase = 0.0f;
        std::memset(ap_buf, 0, sizeof(ap_buf));
        ap_pos = 0;
    }

    void process(
        const float *const *inputs,
        float **outputs,
        uint32_t num_samples,
        const ParamUpdateCmd *pending_params,
        uint32_t num_params) override
    {
        apply_pending_params(pending_params, num_params);
        const float *in = inputs[0];
        float *out = outputs[0];
        if (!in || !out)
        {
            if (out)
                std::memset(out, 0, num_samples * sizeof(float));
            return;
        }

        switch (mode)
        {
        case DISTORTION:
            process_distortion(in, out, num_samples);
            break;
        case OVERDRIVE:
            process_overdrive(in, out, num_samples);
            break;
        case BITCRUSHER:
            process_bitcrusher(in, out, num_samples);
            break;
        case RING_MOD:
            process_ring_mod(in, out, num_samples);
            break;
        case WAVESHAPER:
            process_waveshaper(in, out, num_samples);
            break;
        case TREMOLO:
            process_tremolo(in, out, num_samples);
            break;
        case STEREO_WIDENER:
            process_widener(in, out, num_samples);
            break;
        default:
            std::memcpy(out, in, num_samples * sizeof(float));
            break;
        }
    }

private:
    void process_distortion(const float *in, float *out, uint32_t n)
    {
        for (uint32_t i = 0; i < n; ++i)
        {
            float x = in[i] * drive;
            // Hard clipping tanh
            float distorted = std::tanh(x);
            // Tone filter (LP) — scrub NaN/Inf so a single poisoned
            // input cannot permanently corrupt the IIR state
            // (`tone_lp += tone*(NaN - tone_lp)` would otherwise
            // pin tone_lp to NaN forever).
            if (!std::isfinite(distorted))
                distorted = 0.0f;
            tone_lp += tone * (distorted - tone_lp);
            if (!std::isfinite(tone_lp))
                tone_lp = 0.0f;
            float shaped = tone_lp * tone + distorted * (1.0f - tone);
            float y = in[i] * (1.0f - distort_mix) + shaped * distort_mix;
            if (!std::isfinite(y))
                y = 0.0f;
            out[i] = y;
        }
    }

    void process_overdrive(const float *in, float *out, uint32_t n)
    {
        // Soft clipping - asymmetric
        for (uint32_t i = 0; i < n; ++i)
        {
            float x = in[i] * drive;
            float distorted;
            if (x > 0.0f)
            {
                distorted = 1.0f - std::exp(-x);
            }
            else
            {
                distorted = -1.0f + std::exp(x);
            }
            if (!std::isfinite(distorted))
                distorted = 0.0f;
            tone_lp += tone * (distorted - tone_lp);
            if (!std::isfinite(tone_lp))
                tone_lp = 0.0f;
            float y = in[i] * (1.0f - distort_mix) + tone_lp * distort_mix;
            if (!std::isfinite(y))
                y = 0.0f;
            out[i] = y;
        }
    }

    void process_bitcrusher(const float *in, float *out, uint32_t n)
    {
        float levels = std::pow(2.0f, bit_depth);
        for (uint32_t i = 0; i < n; ++i)
        {
            crush_counter += 1.0f;
            if (crush_counter >= sample_rate_reduce)
            {
                crush_counter -= sample_rate_reduce;
                // Quantize. NaN scrub on the input prevents
                // crush_held from sticking at NaN until the next
                // sample-and-hold tick (which can be hundreds of
                // samples away at high rate-reduction).
                float x = in[i];
                if (!std::isfinite(x))
                    x = 0.0f;
                crush_held = std::round(x * levels) / levels;
                if (!std::isfinite(crush_held))
                    crush_held = 0.0f;
            }
            float y = in[i] * (1.0f - distort_mix) + crush_held * distort_mix;
            if (!std::isfinite(y))
                y = 0.0f;
            out[i] = y;
        }
    }

    void process_ring_mod(const float *in, float *out, uint32_t n)
    {
        float phase_inc = TWO_PI * ring_freq / sample_rate;
        for (uint32_t i = 0; i < n; ++i)
        {
            float carrier = std::sin(ring_phase);
            ring_phase += phase_inc;
            if (ring_phase > TWO_PI)
                ring_phase -= TWO_PI;
            float modulated = in[i] * carrier;
            out[i] = in[i] * (1.0f - ring_mix) + modulated * ring_mix;
        }
    }

    void process_waveshaper(const float *in, float *out, uint32_t n)
    {
        // Chebyshev-based waveshaping
        for (uint32_t i = 0; i < n; ++i)
        {
            float x = in[i];
            float k = 2.0f * ws_amount / (1.0f + ws_amount);
            float shaped = (1.0f + k) * x / (1.0f + k * std::fabs(x));
            out[i] = in[i] * (1.0f - ws_mix) + shaped * ws_mix;
        }
    }

    void process_tremolo(const float *in, float *out, uint32_t n)
    {
        float phase_inc = TWO_PI * trem_rate / sample_rate;
        for (uint32_t i = 0; i < n; ++i)
        {
            float lfo = 0.5f + 0.5f * std::sin(trem_phase);
            trem_phase += phase_inc;
            if (trem_phase > TWO_PI)
                trem_phase -= TWO_PI;
            float gain = 1.0f - trem_depth * lfo;
            out[i] = in[i] * gain;
        }
    }

    void process_widener(const float *in, float *out, uint32_t n)
    {
        // Mono decorrelation via allpass delay
        for (uint32_t i = 0; i < n; ++i)
        {
            float delayed = ap_buf[ap_pos];
            ap_buf[ap_pos] = in[i];
            ap_pos = (ap_pos + 1) % AP_LEN;
            out[i] = in[i] * (1.0f - width) + delayed * width;
        }
    }
};
