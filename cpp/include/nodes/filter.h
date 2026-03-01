/// 2nd-order butterworth low-pass filter.
/// Classic state-variable design with smooth parameter transitions.
///
/// Parameters:
/// - cutoff: Cutoff frequency in Hz (default 5000.0)
/// - resonance: Filter resonance / Q factor (default 0.707)

#pragma once

#include "audio_node.h"
#include <cmath>
#include <algorithm>

class FilterNode : public AudioNode {
private:
    // State variables (direct form II)
    float state_z1 = 0.0f;
    float state_z2 = 0.0f;

    // Filter coefficients
    float b0 = 1.0f, b1 = 0.0f, b2 = 0.0f;
    float a1 = 0.0f, a2 = 0.0f;

    // Target parameters for smoothing
    float cutoff = 5000.0f;
    float resonance = 0.707f;
    float target_cutoff = 5000.0f;
    float target_resonance = 0.707f;

public:
    FilterNode() : AudioNode() {
        num_inputs = 1;
        num_outputs = 1;
        compute_coefficients();
    }

    virtual ~FilterNode() = default;

    void set_param(uint32_t param_hash, float value) override {
        if (param_hash == ParamHash::FILTER_CUTOFF) {
            target_cutoff = std::fmax(10.0f, std::fmin(value, sample_rate * 0.45f));
        } else if (param_hash == ParamHash::FILTER_RESONANCE) {
            target_resonance = std::fmax(0.1f, std::fmin(value, 10.0f));
        }
    }

    void reset() override {
        state_z1 = 0.0f;
        state_z2 = 0.0f;
        cutoff = 5000.0f;
        resonance = 0.707f;
        target_cutoff = 5000.0f;
        target_resonance = 0.707f;
        compute_coefficients();
    }

    void process(
        const float* const* inputs,
        float** outputs,
        uint32_t num_samples,
        const ParamUpdateCmd* pending_params,
        uint32_t num_params
    ) override {
        // Apply pending parameter updates
        apply_pending_params(pending_params, num_params);

        const float* in = inputs[0];
        float* out = outputs[0];

        // If no input connected, output silence
        if (!in || !out) {
            if (out) std::memset(out, 0, num_samples * sizeof(float));
            return;
        }

        // Process each sample with state-variable filter
        for (uint32_t i = 0; i < num_samples; ++i) {
            // Smoothly transition parameters
            cutoff = cutoff * 0.95f + target_cutoff * 0.05f;
            resonance = resonance * 0.95f + target_resonance * 0.05f;
            compute_coefficients();

            // Direct form II biquad
            float y = b0 * in[i] + state_z1;
            state_z1 = b1 * in[i] + state_z2 - a1 * y;
            state_z2 = b2 * in[i] - a2 * y;

            out[i] = y;
        }
    }

private:
    void compute_coefficients() {
        // Butterworth 2nd-order low-pass filter
        // Normalized cutoff frequency [0, 1]
        float wc = cutoff / sample_rate;
        wc = std::fmax(0.001f, std::fmin(wc, 0.499f));  // Clamp to safe range

        // Bilinear transform for butterworth
        float q = resonance;
        float alpha = std::sin(wc * 3.14159265f) / (2.0f * q);
        
        b0 = (1.0f - std::cos(wc * 3.14159265f * 2.0f)) / 2.0f;
        b1 = 1.0f - std::cos(wc * 3.14159265f * 2.0f);
        b2 = (1.0f - std::cos(wc * 3.14159265f * 2.0f)) / 2.0f;

        float a0 = 1.0f + alpha;
        a1 = -2.0f * std::cos(wc * 3.14159265f * 2.0f) / a0;
        a2 = (1.0f - alpha) / a0;

        b0 /= a0;
        b1 /= a0;
        b2 /= a0;
    }
};
