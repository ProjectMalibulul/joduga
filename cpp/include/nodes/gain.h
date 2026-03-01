/// Gain/Volume node.
/// Simple linear amplitude scaling with parameter smoothing.
///
/// Parameters:
/// - gain: Gain in linear scale (default 1.0, range 0.0-2.0)

#pragma once

#include "audio_node.h"
#include <algorithm>

#include <cmath>

class GainNode : public AudioNode
{
private:
    float gain = 1.0f;
    float target_gain = 1.0f;

public:
    GainNode() : AudioNode()
    {
        num_inputs = 1;
        num_outputs = 1;
    }

    virtual ~GainNode() = default;

    void set_param(uint32_t param_hash, float value) override
    {
        if (param_hash == ParamHash::GAIN_LEVEL || param_hash == ParamHash::FREQ)
        {
            target_gain = std::fmax(0.0f, std::fmin(value, 10.0f));
        }
    }

    void reset() override
    {
        gain = 1.0f;
        target_gain = 1.0f;
    }

    void process(
        const float *const *inputs,
        float **outputs,
        uint32_t num_samples,
        const ParamUpdateCmd *pending_params,
        uint32_t num_params) override
    {
        // Apply pending parameter updates
        apply_pending_params(pending_params, num_params);

        const float *in = inputs[0];
        float *out = outputs[0];

        // If no input connected, output silence
        if (!in || !out)
        {
            if (out)
                std::memset(out, 0, num_samples * sizeof(float));
            return;
        }

        for (uint32_t i = 0; i < num_samples; ++i)
        {
            // Smooth gain interpolation to avoid clicks
            gain = gain * 0.99f + target_gain * 0.01f;
            out[i] = in[i] * gain;
        }
    }
};
