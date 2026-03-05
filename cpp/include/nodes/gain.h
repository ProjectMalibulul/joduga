/// Gain/Volume/Dynamics node.
/// Supports: simple gain, compressor, limiter, gate, expander.
///
/// The dynamics mode is selected via GAIN_MODE param (set at init).
/// Default mode (0) is simple gain/mixer.

#pragma once

#include "audio_node.h"
#include <algorithm>
#include <cmath>
#include <cstring>

class GainNode : public AudioNode
{
public:
    enum Mode
    {
        SIMPLE_GAIN = 0,
        COMPRESSOR = 1,
        LIMITER = 2,
        GATE = 3,
        EXPANDER = 4,
    };

private:
    int mode = SIMPLE_GAIN;
    float gain = 1.0f;
    float target_gain = 1.0f;

    // Dynamics parameters
    float threshold_db = -20.0f;
    float ratio = 4.0f;
    float attack_ms = 10.0f;
    float release_ms = 100.0f;
    float knee_db = 6.0f;
    float makeup_db = 0.0f;

    // Envelope follower state
    float env_db = -96.0f;
    float gain_reduction_db = 0.0f;

public:
    GainNode() : AudioNode()
    {
        num_inputs = 1;
        num_outputs = 1;
    }

    virtual ~GainNode() = default;

    void set_param(uint32_t param_hash, float value) override
    {
        switch (param_hash)
        {
        case ParamHash::GAIN_MODE:
            mode = static_cast<int>(value);
            break;
        case ParamHash::GAIN_LEVEL: // = FREQ = 0x811C9DC5
            target_gain = std::fmax(0.0f, std::fmin(value, 10.0f));
            break;
        case ParamHash::THRESHOLD:
            threshold_db = std::fmax(-96.0f, std::fmin(value, 0.0f));
            break;
        case ParamHash::RATIO:
            ratio = std::fmax(1.0f, std::fmin(value, 100.0f));
            break;
        case ParamHash::ATTACK:
            attack_ms = std::fmax(0.01f, std::fmin(value, 1000.0f));
            break;
        case ParamHash::RELEASE:
            release_ms = std::fmax(1.0f, std::fmin(value, 5000.0f));
            break;
        case ParamHash::DYN_KNEE:
            knee_db = std::fmax(0.0f, std::fmin(value, 24.0f));
            break;
        case ParamHash::DYN_MAKEUP:
            makeup_db = std::fmax(-12.0f, std::fmin(value, 48.0f));
            break;
        }
    }

    void reset() override
    {
        gain = 1.0f;
        target_gain = 1.0f;
        env_db = -96.0f;
        gain_reduction_db = 0.0f;
    }

    void process(
        const float *const *inputs,
        float **outputs,
        uint32_t num_samples,
        const ParamUpdateCmd *pending_params,
        uint32_t num_params) override
    {
        apply_pending_params(pending_params, num_params);

        float *out = outputs[0];
        if (!out)
            return;

        // Sum all connected inputs
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

        if (!any_input)
            return;

        switch (mode)
        {
        case COMPRESSOR:
        case LIMITER:
            process_compressor(out, num_samples);
            break;
        case GATE:
        case EXPANDER:
            process_gate(out, num_samples);
            break;
        default:
            // Simple gain with smoothing
            for (uint32_t i = 0; i < num_samples; ++i)
            {
                gain = gain * 0.99f + target_gain * 0.01f;
                out[i] *= gain;
            }
            break;
        }
    }

private:
    static float db_to_lin(float db)
    {
        return std::pow(10.0f, db / 20.0f);
    }

    static float lin_to_db(float lin)
    {
        return 20.0f * std::log10(std::fmax(lin, 1e-10f));
    }

    void process_compressor(float *buf, uint32_t n)
    {
        float attack_coeff = std::exp(-1.0f / (attack_ms * 0.001f * sample_rate));
        float release_coeff = std::exp(-1.0f / (release_ms * 0.001f * sample_rate));
        float eff_ratio = (mode == LIMITER) ? 100.0f : ratio;
        float makeup_lin = db_to_lin(makeup_db);

        for (uint32_t i = 0; i < n; ++i)
        {
            float abs_val = std::fabs(buf[i]);
            float input_db = lin_to_db(abs_val);

            // Envelope follower
            if (input_db > env_db)
                env_db = attack_coeff * env_db + (1.0f - attack_coeff) * input_db;
            else
                env_db = release_coeff * env_db + (1.0f - release_coeff) * input_db;

            // Gain computation with soft knee
            float overshoot = env_db - threshold_db;
            float gr = 0.0f;
            if (knee_db > 0.0f && overshoot > -knee_db * 0.5f && overshoot < knee_db * 0.5f)
            {
                float x = overshoot + knee_db * 0.5f;
                gr = x * x / (2.0f * knee_db) * (1.0f - 1.0f / eff_ratio);
            }
            else if (overshoot > 0.0f)
            {
                gr = overshoot * (1.0f - 1.0f / eff_ratio);
            }

            gain_reduction_db = gr;
            float gain_lin = db_to_lin(-gain_reduction_db) * makeup_lin;
            buf[i] *= gain_lin;
        }
    }

    void process_gate(float *buf, uint32_t n)
    {
        float attack_coeff = std::exp(-1.0f / (attack_ms * 0.001f * sample_rate));
        float release_coeff = std::exp(-1.0f / (release_ms * 0.001f * sample_rate));
        float eff_ratio = (mode == EXPANDER) ? ratio : 100.0f;

        for (uint32_t i = 0; i < n; ++i)
        {
            float abs_val = std::fabs(buf[i]);
            float input_db = lin_to_db(abs_val);

            if (input_db > env_db)
                env_db = attack_coeff * env_db + (1.0f - attack_coeff) * input_db;
            else
                env_db = release_coeff * env_db + (1.0f - release_coeff) * input_db;

            // Below threshold: attenuate
            float undershoot = threshold_db - env_db;
            float gr = 0.0f;
            if (undershoot > 0.0f)
            {
                gr = undershoot * (1.0f - 1.0f / eff_ratio);
            }

            float gain_lin = db_to_lin(-gr);
            buf[i] *= gain_lin;
        }
    }
};
