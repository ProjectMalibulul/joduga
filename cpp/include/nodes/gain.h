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

        float *out = outputs[0];
        if (!out)
            return;

        // Sum all connected inputs into the output buffer.
        // This makes GainNode work as a mixer when multiple inputs
        // are connected (e.g. "Mixer 2-Ch" has num_inputs=2).
        bool any_input = false;
        std::memset(out, 0, num_samples * sizeof(float));

        for (uint32_t ch = 0; ch < num_inputs; ++ch)
        {
            const float *in = inputs[ch];
            if (!in)
                continue;
            any_input = true;
            for (uint32_t i = 0; i < num_samples; ++i)
                out[i] += in[i];
        }

        // If nothing was connected, output stays silent
        if (!any_input)
            return;

        // Apply gain with smoothing
        for (uint32_t i = 0; i < num_samples; ++i)
        {
            gain = gain * 0.99f + target_gain * 0.01f;
            out[i] *= gain;
        }
    }
};
