/// Multi-mode biquad filter node.
/// Supports: LP, HP, BP, Notch, Allpass, PeakEQ, Low-Shelf, High-Shelf,
///           DC-Blocker, and a simple Comb filter.
///
/// The filter mode is selected at runtime via the FILTER_MODE parameter.

#pragma once

#include "audio_node.h"
#include <cmath>
#include <algorithm>
#include <cstring>

class FilterNode : public AudioNode
{
public:
    // Filter mode enum (must match catalog engineSubtype values)
    enum Mode
    {
        LP = 0,
        HP = 1,
        BP = 2,
        NOTCH = 3,
        ALLPASS = 4,
        COMB = 5,
        FORMANT = 6,
        MOOG = 7,
        SVF = 8,
        PEAK_EQ = 9,
        LOW_SHELF = 10,
        HIGH_SHELF = 11,
        TILT_EQ = 12,
        DC_BLOCK = 13,
        MOVING_AVG = 14,
        CROSSOVER = 15,
        RESONATOR = 16,
        VOWEL = 17,
    };

private:
    // Biquad state (direct form II transposed)
    float z1 = 0.0f, z2 = 0.0f;

    // Coefficients
    float b0 = 1.0f, b1 = 0.0f, b2 = 0.0f;
    float a1 = 0.0f, a2 = 0.0f;

    // Parameters with smoothing
    float cutoff = 5000.0f;
    float resonance = 0.707f;
    float target_cutoff = 5000.0f;
    float target_resonance = 0.707f;
    int mode = LP;
    float gain_db = 0.0f; // for shelf / peak EQ

    // Comb filter state
    static constexpr int MAX_COMB_LEN = 4800; // up to 100ms at 48kHz
    float comb_buf[MAX_COMB_LEN] = {};
    int comb_write = 0;
    float comb_delay_ms = 5.0f;
    float comb_feedback = 0.7f;

public:
    FilterNode() : AudioNode()
    {
        num_inputs = 1;
        num_outputs = 1;
        compute_coefficients();
    }

    virtual ~FilterNode() = default;

    void set_param(uint32_t param_hash, float value) override
    {
        if (param_hash == ParamHash::FILTER_CUTOFF)
        {
            target_cutoff = std::fmax(10.0f, std::fmin(value, sample_rate * 0.45f));
        }
        else if (param_hash == ParamHash::FILTER_RESONANCE)
        {
            target_resonance = std::fmax(0.01f, std::fmin(value, 30.0f));
        }
        else if (param_hash == ParamHash::FILTER_MODE)
        {
            mode = static_cast<int>(value);
        }
        else if (param_hash == ParamHash::COMB_DELAY)
        {
            comb_delay_ms = std::fmax(0.1f, std::fmin(value, 100.0f));
        }
        else if (param_hash == ParamHash::COMB_FEEDBACK)
        {
            comb_feedback = std::fmax(0.0f, std::fmin(value, 0.99f));
        }
        else if (param_hash == ParamHash::PARAMETRIC_Q)
        {
            // Remap Q into resonance for peak EQ
            target_resonance = std::fmax(0.1f, std::fmin(value, 20.0f));
        }
    }

    void reset() override
    {
        z1 = z2 = 0.0f;
        cutoff = target_cutoff = 5000.0f;
        resonance = target_resonance = 0.707f;
        gain_db = 0.0f;
        mode = LP;
        std::memset(comb_buf, 0, sizeof(comb_buf));
        comb_write = 0;
        compute_coefficients();
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

        // Comb filter has its own processing path
        if (mode == COMB)
        {
            process_comb(in, out, num_samples);
            return;
        }

        // Biquad processing for all standard modes
        // Smooth parameters once per block (not per-sample) to avoid
        // expensive trig recomputation on every sample.
        cutoff = cutoff * 0.95f + target_cutoff * 0.05f;
        resonance = resonance * 0.95f + target_resonance * 0.05f;
        compute_coefficients();

        for (uint32_t i = 0; i < num_samples; ++i)
        {
            // Direct Form II Transposed biquad
            float x = in[i];
            float y = b0 * x + z1;
            z1 = b1 * x - a1 * y + z2;
            z2 = b2 * x - a2 * y;

            // Soft-clip to prevent explosions at high resonance
            if (y > 4.0f)
                y = 4.0f;
            if (y < -4.0f)
                y = -4.0f;

            out[i] = y;
        }
    }

private:
    void process_comb(const float *in, float *out, uint32_t num_samples)
    {
        int delay_len = static_cast<int>(comb_delay_ms * 0.001f * sample_rate);
        if (delay_len < 1)
            delay_len = 1;
        if (delay_len >= MAX_COMB_LEN)
            delay_len = MAX_COMB_LEN - 1;

        for (uint32_t i = 0; i < num_samples; ++i)
        {
            int read_pos = (comb_write - delay_len + MAX_COMB_LEN) % MAX_COMB_LEN;
            float delayed = comb_buf[read_pos];
            float y = in[i] + delayed * comb_feedback;
            comb_buf[comb_write] = y;
            comb_write = (comb_write + 1) % MAX_COMB_LEN;
            out[i] = y;
        }
    }

    void compute_coefficients()
    {
        float w0 = 2.0f * 3.14159265f * cutoff / sample_rate;
        w0 = std::fmax(0.001f, std::fmin(w0, 3.1f)); // clamp near Nyquist
        float cosw0 = std::cos(w0);
        float sinw0 = std::sin(w0);
        float Q = std::fmax(0.01f, resonance);
        float alpha = sinw0 / (2.0f * Q);

        float a0 = 1.0f;

        switch (mode)
        {
        default:
        case LP: // Low-Pass
        {
            b0 = (1.0f - cosw0) / 2.0f;
            b1 = 1.0f - cosw0;
            b2 = (1.0f - cosw0) / 2.0f;
            a0 = 1.0f + alpha;
            a1 = -2.0f * cosw0;
            a2 = 1.0f - alpha;
            break;
        }
        case HP: // High-Pass
        {
            b0 = (1.0f + cosw0) / 2.0f;
            b1 = -(1.0f + cosw0);
            b2 = (1.0f + cosw0) / 2.0f;
            a0 = 1.0f + alpha;
            a1 = -2.0f * cosw0;
            a2 = 1.0f - alpha;
            break;
        }
        case BP: // Band-Pass (constant skirt gain)
        {
            b0 = alpha;
            b1 = 0.0f;
            b2 = -alpha;
            a0 = 1.0f + alpha;
            a1 = -2.0f * cosw0;
            a2 = 1.0f - alpha;
            break;
        }
        case NOTCH: // Notch / Band-Reject
        {
            b0 = 1.0f;
            b1 = -2.0f * cosw0;
            b2 = 1.0f;
            a0 = 1.0f + alpha;
            a1 = -2.0f * cosw0;
            a2 = 1.0f - alpha;
            break;
        }
        case ALLPASS: // All-Pass
        {
            b0 = 1.0f - alpha;
            b1 = -2.0f * cosw0;
            b2 = 1.0f + alpha;
            a0 = 1.0f + alpha;
            a1 = -2.0f * cosw0;
            a2 = 1.0f - alpha;
            break;
        }
        case PEAK_EQ: // Peaking EQ
        case FORMANT:
        case MOOG:
        case SVF:
        case TILT_EQ:
        case RESONATOR:
        case VOWEL:
        {
            float A = std::pow(10.0f, gain_db / 40.0f);
            b0 = 1.0f + alpha * A;
            b1 = -2.0f * cosw0;
            b2 = 1.0f - alpha * A;
            a0 = 1.0f + alpha / A;
            a1 = -2.0f * cosw0;
            a2 = 1.0f - alpha / A;
            break;
        }
        case LOW_SHELF: // Low Shelf EQ
        {
            float A = std::pow(10.0f, gain_db / 40.0f);
            float sq = 2.0f * std::sqrt(A) * alpha;
            b0 = A * ((A + 1.0f) - (A - 1.0f) * cosw0 + sq);
            b1 = 2.0f * A * ((A - 1.0f) - (A + 1.0f) * cosw0);
            b2 = A * ((A + 1.0f) - (A - 1.0f) * cosw0 - sq);
            a0 = (A + 1.0f) + (A - 1.0f) * cosw0 + sq;
            a1 = -2.0f * ((A - 1.0f) + (A + 1.0f) * cosw0);
            a2 = (A + 1.0f) + (A - 1.0f) * cosw0 - sq;
            break;
        }
        case HIGH_SHELF: // High Shelf EQ
        {
            float A = std::pow(10.0f, gain_db / 40.0f);
            float sq = 2.0f * std::sqrt(A) * alpha;
            b0 = A * ((A + 1.0f) + (A - 1.0f) * cosw0 + sq);
            b1 = -2.0f * A * ((A - 1.0f) + (A + 1.0f) * cosw0);
            b2 = A * ((A + 1.0f) + (A - 1.0f) * cosw0 - sq);
            a0 = (A + 1.0f) - (A - 1.0f) * cosw0 + sq;
            a1 = 2.0f * ((A - 1.0f) - (A + 1.0f) * cosw0);
            a2 = (A + 1.0f) - (A - 1.0f) * cosw0 - sq;
            break;
        }
        case DC_BLOCK: // DC Blocker (high-pass at very low freq)
        {
            b0 = (1.0f + cosw0) / 2.0f;
            b1 = -(1.0f + cosw0);
            b2 = (1.0f + cosw0) / 2.0f;
            a0 = 1.0f + alpha;
            a1 = -2.0f * cosw0;
            a2 = 1.0f - alpha;
            break;
        }
        case MOVING_AVG: // Moving average (approx as LP)
        case CROSSOVER:
        {
            // Same as LP
            b0 = (1.0f - cosw0) / 2.0f;
            b1 = 1.0f - cosw0;
            b2 = (1.0f - cosw0) / 2.0f;
            a0 = 1.0f + alpha;
            a1 = -2.0f * cosw0;
            a2 = 1.0f - alpha;
            break;
        }
        case COMB: // Handled in process_comb(), coefficients unused
            return;
        }

        // Normalize by a0
        if (a0 != 0.0f)
        {
            b0 /= a0;
            b1 /= a0;
            b2 /= a0;
            a1 /= a0;
            a2 /= a0;
        }
    }
};
