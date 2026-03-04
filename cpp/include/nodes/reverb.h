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
        set_delay_lengths(room_size);
    }

    void set_param(uint32_t param_hash, float value) override
    {
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

            const float f0 = x + feedback * (d1 + d2 - d3);
            const float f1 = x + feedback * (d0 - d2 + d3);
            const float f2 = x + feedback * (-d0 + d1 + d3);
            const float f3 = x + feedback * (d0 + d1 - d2);

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
            n = std::max<size_t>(32, std::min<size_t>(n, 96000));
            delay_len[i] = n;
            lines[i].assign(n, 0.0f);
            write_pos[i] = 0;
        }
    }
};
