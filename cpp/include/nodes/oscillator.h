/// Multi-waveform oscillator DSP node.
/// Supports: sine, square, sawtooth, triangle, white/pink/brown noise,
///           FM, AM, and basic additive synthesis.
///
/// The waveform type is selected via the WAVEFORM_TYPE parameter.

#pragma once

#include "audio_node.h"
#include <algorithm>
#include <cmath>
#include <cstdlib>

class OscillatorNode : public AudioNode
{
public:
    enum Waveform
    {
        SINE = 0,
        SQUARE = 1,
        SAW = 2,
        TRIANGLE = 3,
        WHITE_NOISE = 4,
        PINK_NOISE = 5,
        BROWN_NOISE = 6,
        FM = 7,
        AM = 8,
        WAVETABLE = 9,
        SUB = 10,
        SUPER_SAW = 11,
        ADDITIVE = 12,
    };

private:
    float phase = 0.0f;
    float frequency = 440.0f;
    float phase_increment = 0.0f;
    float sample_rate_inv = 1.0f / 48000.0f;

    float target_frequency = 440.0f;
    float frequency_coeff = 0.999f;

    int waveform = SINE;
    float duty_cycle = 0.5f;

    // FM/AM parameters
    float mod_depth = 1.0f;
    float mod_freq = 5.0f;
    float mod_phase = 0.0f;

    // Super Saw
    float detune = 0.3f;
    int voices = 5;
    float saw_phases[7] = {};

    // Additive
    int harmonics = 8;
    float rolloff = 1.0f;

    // Noise state
    float brown_state = 0.0f;
    float pink_b[7] = {};

    // Random seed
    uint32_t rng_state = 12345u;

public:
    OscillatorNode() : AudioNode()
    {
        num_inputs = 0;
        num_outputs = 1;
        update_phase_increment();
    }

    virtual ~OscillatorNode() = default;

    void set_param(uint32_t param_hash, float value) override
    {
        if (param_hash == ParamHash::OSC_FREQUENCY || param_hash == ParamHash::FREQ)
        {
            target_frequency = std::fmax(0.01f, std::fmin(value, 20000.0f));
        }
        else if (param_hash == ParamHash::WAVEFORM_TYPE)
        {
            waveform = static_cast<int>(value);
        }
        else if (param_hash == ParamHash::DUTY_CYCLE)
        {
            duty_cycle = std::fmax(0.01f, std::fmin(value, 0.99f));
        }
        else if (param_hash == ParamHash::FM_MOD_DEPTH)
        {
            // Phase-modulation depth in radians; large values run into
            // aliasing but won't blow up the mod_phase accumulator.
            mod_depth = std::fmax(0.0f, std::fmin(value, 100.0f));
        }
        else if (param_hash == ParamHash::FM_MOD_FREQ)
        {
            // Clamp into the audible range. An unclamped huge value
            // would make `mod_phase += TWO_PI * mod_freq * dt` exceed
            // TWO_PI per sample, defeating the single-step wrap below
            // and letting mod_phase grow without bound — sin() on huge
            // floats then loses precision and the output decays to
            // shaped garbage.
            mod_freq = std::fmax(0.0f, std::fmin(value, 20000.0f));
        }
        else if (param_hash == ParamHash::AM_MOD_DEPTH)
        {
            mod_depth = std::fmax(0.0f, std::fmin(value, 100.0f));
        }
        else if (param_hash == ParamHash::AM_MOD_FREQ)
        {
            mod_freq = std::fmax(0.0f, std::fmin(value, 20000.0f));
        }
        else if (param_hash == ParamHash::DETUNE)
        {
            // Detune is a 0-1 ratio scaled internally by 0.01 per voice.
            // An unclamped large value combined with high `voices` and a
            // 20 kHz carrier could push `saw_phases[j] += TWO_PI * f *
            // (1 + detune_amt) * dt` past TWO_PI per sample, defeating
            // the single-step wrap below.
            detune = std::fmax(0.0f, std::fmin(value, 1.0f));
        }
        else if (param_hash == 0xAAu)
        { // voices
            voices = std::max(1, std::min(static_cast<int>(value), 7));
        }
        else if (param_hash == 0xABu)
        { // harmonics
            harmonics = std::max(1, std::min(static_cast<int>(value), 32));
        }
        else if (param_hash == 0xACu)
        { // rolloff
            rolloff = value;
        }
    }

    void reset() override
    {
        phase = 0.0f;
        mod_phase = 0.0f;
        frequency = 440.0f;
        target_frequency = 440.0f;
        brown_state = 0.0f;
        std::memset(pink_b, 0, sizeof(pink_b));
        std::memset(saw_phases, 0, sizeof(saw_phases));
        update_phase_increment();
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
        for (uint32_t i = 0; i < num_samples; ++i)
        {
            // Smooth frequency
            frequency = frequency * frequency_coeff + target_frequency * (1.0f - frequency_coeff);
            update_phase_increment();

            float sample = 0.0f;

            switch (waveform)
            {
            default:
            case SINE:
                sample = std::sin(phase);
                break;

            case SQUARE:
                sample = (phase < TWO_PI * duty_cycle) ? 1.0f : -1.0f;
                break;

            case SAW:
                sample = 2.0f * (phase / TWO_PI) - 1.0f;
                break;

            case TRIANGLE:
            {
                float t = phase / TWO_PI;
                sample = (t < 0.5f) ? (4.0f * t - 1.0f) : (3.0f - 4.0f * t);
                break;
            }

            case WHITE_NOISE:
                sample = random_float() * 2.0f - 1.0f;
                break;

            case PINK_NOISE:
                sample = generate_pink_noise();
                break;

            case BROWN_NOISE:
            {
                brown_state += (random_float() * 2.0f - 1.0f) * 0.02f;
                brown_state *= 0.999f;
                sample = brown_state * 10.0f;
                sample = std::fmax(-1.0f, std::fmin(sample, 1.0f));
                break;
            }

            case FM:
            {
                float mod = mod_depth * std::sin(mod_phase);
                sample = std::sin(phase + mod);
                mod_phase += TWO_PI * mod_freq * sample_rate_inv;
                while (mod_phase > TWO_PI)
                    mod_phase -= TWO_PI;
                break;
            }

            case AM:
            {
                float carrier = std::sin(phase);
                float mod = 1.0f + mod_depth * std::sin(mod_phase);
                sample = carrier * mod * 0.5f;
                mod_phase += TWO_PI * mod_freq * sample_rate_inv;
                while (mod_phase > TWO_PI)
                    mod_phase -= TWO_PI;
                break;
            }

            case WAVETABLE:
            case SUB:
                // Sub: just sine at the (possibly lower) frequency
                sample = std::sin(phase);
                break;

            case SUPER_SAW:
            {
                sample = 0.0f;
                int v = std::min(voices, 7);
                for (int j = 0; j < v; ++j)
                {
                    float detune_amt = (j - v / 2) * detune * 0.01f;
                    float p = saw_phases[j];
                    sample += 2.0f * (p / TWO_PI) - 1.0f;
                    saw_phases[j] += TWO_PI * frequency * (1.0f + detune_amt) * sample_rate_inv;
                    while (saw_phases[j] > TWO_PI)
                        saw_phases[j] -= TWO_PI;
                }
                sample /= (float)v;
                break;
            }

            case ADDITIVE:
            {
                sample = 0.0f;
                float amp = 1.0f;
                for (int h = 1; h <= harmonics; ++h)
                {
                    sample += amp * std::sin(phase * h);
                    amp /= (1.0f + rolloff);
                }
                sample /= (float)harmonics * 0.5f;
                sample = std::fmax(-1.0f, std::fmin(sample, 1.0f));
                break;
            }
            }

            out[i] = sample;

            // Advance phase
            phase += phase_increment;
            if (phase > TWO_PI)
                phase -= TWO_PI;
        }
    }

private:
    void update_phase_increment()
    {
        phase_increment = TWO_PI * frequency * sample_rate_inv;
    }

    float random_float()
    {
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 17;
        rng_state ^= rng_state << 5;
        return static_cast<float>(rng_state) / 4294967296.0f;
    }

    float generate_pink_noise()
    {
        float white = random_float() * 2.0f - 1.0f;
        pink_b[0] = 0.99886f * pink_b[0] + white * 0.0555179f;
        pink_b[1] = 0.99332f * pink_b[1] + white * 0.0750759f;
        pink_b[2] = 0.96900f * pink_b[2] + white * 0.1538520f;
        pink_b[3] = 0.86650f * pink_b[3] + white * 0.3104856f;
        pink_b[4] = 0.55000f * pink_b[4] + white * 0.5329522f;
        pink_b[5] = -0.7616f * pink_b[5] - white * 0.0168980f;
        float pink = pink_b[0] + pink_b[1] + pink_b[2] + pink_b[3] + pink_b[4] + pink_b[5] + pink_b[6] + white * 0.5362f;
        pink_b[6] = white * 0.115926f;
        return pink * 0.11f; // normalize
    }
};
