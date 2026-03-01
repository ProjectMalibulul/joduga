/// Sine oscillator DSP node.
/// Simple, efficient sine wave generator with frequency modulation.
///
/// Parameters:
/// - frequency: Oscillation frequency in Hz (default 440.0)

#pragma once

#include "audio_node.h"
#include <cmath>

class OscillatorNode : public AudioNode {
private:
    float phase = 0.0f;           // Current phase [0, 2π)
    float frequency = 440.0f;     // Frequency in Hz
    float phase_increment = 0.0f; // Phase step per sample
    float sample_rate_inv = 1.0f / 48000.0f;

    // Cached values for parameter smoothing
    float target_frequency = 440.0f;
    float frequency_coeff = 0.999f;  // Smoothing coefficient

public:
    OscillatorNode() : AudioNode() {
        num_inputs = 0;
        num_outputs = 1;
        update_phase_increment();
    }

    virtual ~OscillatorNode() = default;

    void set_param(uint32_t param_hash, float value) override {
        if (param_hash == ParamHash::OSC_FREQUENCY) {
            // Clamp frequency to reasonable range
            target_frequency = std::fmax(0.1f, std::fmin(value, 20000.0f));
        }
    }

    void reset() override {
        phase = 0.0f;
        frequency = 440.0f;
        target_frequency = 440.0f;
        update_phase_increment();
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

        // Smoothly interpolate frequency to avoid clicks
        float* out = outputs[0];
        for (uint32_t i = 0; i < num_samples; ++i) {
            // Smooth frequency transition
            frequency = frequency * frequency_coeff + target_frequency * (1.0f - frequency_coeff);
            update_phase_increment();

            // Generate sine sample
            out[i] = std::sin(phase);

            // Advance phase
            phase += phase_increment;
            
            // Wrap phase to [0, 2π)
            if (phase > TWO_PI) {
                phase -= TWO_PI;
            }
        }
    }

private:
    void update_phase_increment() {
        phase_increment = TWO_PI * frequency * sample_rate_inv;
    }
};
