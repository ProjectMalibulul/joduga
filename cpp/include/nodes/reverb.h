/// Reverb node (skeletal FDN implementation).
///
/// A lightweight 4-delay feedback delay network suitable as a starting point.

#pragma once

#include "audio_node.h"

#include <algorithm>
#include <array>
#include <cstddef>
#include <cmath>
#include <cstring>
#include <vector>

class ReverbNode : public AudioNode
{
private:
    static constexpr size_t FDN_SIZE = 4;
    static constexpr size_t MAX_DELAY_SAMPLES = 96000; // ≥1 s at 96 kHz
    std::array<std::vector<float>, FDN_SIZE> lines;
    std::array<size_t, FDN_SIZE> write_pos{};
    std::array<size_t, FDN_SIZE> delay_len{};

    float room_size = 0.5f;
    float feedback = 0.65f;
    float wet = 0.25f;

public:
    ReverbNode() : AudioNode()
    {
        num_inputs = 1;
        num_outputs = 1;
        // Allocate the maximum buffer once at construction so subsequent
        // DELAY_TIME / room-size param changes never reallocate from
        // the audio thread (priority inversion → xrun).
        for (auto &line : lines)
            line.assign(MAX_DELAY_SAMPLES, 0.0f);
        set_delay_lengths(room_size);
    }

    void set_param(uint32_t param_hash, float value) override
    {
        if (!std::isfinite(value))
            return;
        switch (param_hash)
        {
        case ParamHash::DELAY_TIME:
            room_size = std::fmax(0.1f, std::fmin(value, 1.0f));
            set_delay_lengths(room_size);
            break;
        case ParamHash::DELAY_FEEDBACK:
            feedback = std::fmax(0.0f, std::fmin(value, 0.95f));
            break;
        case ParamHash::MIX:
            wet = std::fmax(0.0f, std::fmin(value, 1.0f));
            break;
        default:
            break;
        }
    }

    void reset() override
    {
        for (auto &line : lines)
            std::fill(line.begin(), line.end(), 0.0f);
        write_pos.fill(0);
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
        if (!out)
            return;
        if (!in)
        {
            std::memset(out, 0, num_samples * sizeof(float));
            return;
        }

        for (uint32_t i = 0; i < num_samples; ++i)
        {
            const float x = in[i];

            float d0 = lines[0][write_pos[0]];
            float d1 = lines[1][write_pos[1]];
            float d2 = lines[2][write_pos[2]];
            float d3 = lines[3][write_pos[3]];

            const float y = 0.25f * (d0 + d1 + d2 + d3);
            out[i] = x * (1.0f - wet) + y * wet;

            float f0 = x + feedback * (d1 + d2 - d3);
            float f1 = x + feedback * (d0 - d2 + d3);
            float f2 = x + feedback * (-d0 + d1 + d3);
            float f3 = x + feedback * (d0 + d1 - d2);

            // NaN/Inf recovery: a single poisoned sample (from an
            // upstream node or a denormal cascade) would otherwise
            // cycle through the FDN matrix forever, leaving the
            // reverb permanently silent or shaped-garbage.
            if (!std::isfinite(f0) || !std::isfinite(f1) || !std::isfinite(f2) ||
                !std::isfinite(f3))
            {
                f0 = f1 = f2 = f3 = 0.0f;
                if (!std::isfinite(out[i]))
                    out[i] = 0.0f;
            }

            lines[0][write_pos[0]] = f0;
            lines[1][write_pos[1]] = f1;
            lines[2][write_pos[2]] = f2;
            lines[3][write_pos[3]] = f3;

            for (size_t line_idx = 0; line_idx < FDN_SIZE; ++line_idx)
                write_pos[line_idx] = (write_pos[line_idx] + 1) % delay_len[line_idx];
        }
    }

private:
    void set_delay_lengths(float room)
    {
        const float base_sec = 0.02f + room * 0.08f;
        const std::array<float, FDN_SIZE> ratios = {1.0f, 1.31f, 1.73f, 2.11f};

        for (size_t i = 0; i < FDN_SIZE; ++i)
        {
            size_t n = static_cast<size_t>(base_sec * ratios[i] * sample_rate);
            n = std::max<size_t>(32, std::min<size_t>(n, MAX_DELAY_SAMPLES));
            delay_len[i] = n;
            // Don't `assign()` here — that would reallocate when called
            // from set_param() on the audio thread. The buffer was
            // sized to MAX_DELAY_SAMPLES in the constructor; we just
            // zero the now-active region so old echoes from the
            // previous room size don't bleed into the new geometry,
            // and clamp write_pos back into range.
            std::fill(lines[i].begin(), lines[i].begin() + n, 0.0f);
            write_pos[i] = 0;
        }
    }
};
