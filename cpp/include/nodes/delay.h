/// Delay-based effects node.
/// Supports: simple delay, reverb (Schroeder), chorus, flanger, phaser, vibrato.
///
/// The effect mode is selected via the DELAY_MODE parameter (set at init).

#pragma once

#include "audio_node.h"
#include <algorithm>
#include <cmath>
#include <cstring>

class DelayNode : public AudioNode
{
public:
    enum Mode
    {
        SIMPLE_DELAY = 0,
        REVERB = 1,
        CHORUS = 2,
        FLANGER = 3,
        PHASER = 4,
        VIBRATO = 5,
        PITCH_SHIFT = 6,
    };

private:
    int mode = SIMPLE_DELAY;

    // Shared delay buffer (max ~2s at 48kHz)
    static constexpr int MAX_DELAY_LEN = 96000;
    float delay_buf[MAX_DELAY_LEN] = {};
    int write_pos = 0;

    // Delay params
    float delay_time_ms = 250.0f;
    float feedback = 0.5f;
    float mix = 0.5f;

    // Reverb (Schroeder 4-comb + 2-allpass)
    static constexpr int NUM_COMBS = 4;
    static constexpr int NUM_APS = 2;
    float comb_bufs[NUM_COMBS][4800] = {};
    int comb_lens[NUM_COMBS] = {1116, 1188, 1277, 1356};
    int comb_pos[NUM_COMBS] = {};
    float comb_fb = 0.84f;
    float ap_bufs[NUM_APS][556] = {};
    int ap_lens[NUM_APS] = {225, 556};
    int ap_pos[NUM_APS] = {};
    float ap_g = 0.5f;
    float reverb_size = 0.6f;
    float reverb_damp = 0.5f;
    float reverb_mix = 0.3f;
    float reverb_lp = 0.0f;

    // Modulation (chorus/flanger/phaser/vibrato)
    float lfo_phase = 0.0f;
    float lfo_rate = 1.0f;
    float lfo_depth = 0.5f;
    float mod_feedback = 0.0f;

    // Phaser all-pass stages
    static constexpr int MAX_PHASER_STAGES = 12;
    float phaser_ap[MAX_PHASER_STAGES] = {};
    int phaser_stages = 4;

    // Pitch shift
    float pitch_semitones = 0.0f;
    float pitch_mix = 1.0f;

public:
    DelayNode() : AudioNode()
    {
        num_inputs = 1;
        num_outputs = 1;
    }
    virtual ~DelayNode() = default;

    void set_param(uint32_t param_hash, float value) override
    {
        if (!std::isfinite(value))
            return;
        switch (param_hash)
        {
        // Delay mode selector
        case 0x000000CDu: // DELAY_MODE
        {
            // Guard against `static_cast<int>(NaN)` UB and out-of-range
            // mode values. Already-finite due to the early-return above,
            // but still clamp to the declared enum range so the
            // `default: memcpy(out, in)` arm in process() is the only
            // bypass path for unknown modes.
            int m = static_cast<int>(value);
            if (m < 0)
                m = 0;
            else if (m > PITCH_SHIFT)
                m = PITCH_SHIFT;
            mode = m;
            break;
        }
        // Simple delay
        case ParamHash::DELAY_TIME:
            delay_time_ms = std::fmax(1.0f, std::fmin(value, 2000.0f));
            break;
        case ParamHash::DELAY_FEEDBACK:
            feedback = std::fmax(0.0f, std::fmin(value, 0.99f));
            break;
        case ParamHash::MIX:
            mix = std::fmax(0.0f, std::fmin(value, 1.0f));
            break;
        // Reverb
        case 0xD4u:
            reverb_size = std::fmax(0.0f, std::fmin(value, 1.0f));
            update_reverb_params();
            break;
        case 0xD5u:
            reverb_damp = std::fmax(0.0f, std::fmin(value, 1.0f));
            break;
        case 0xD6u:
            reverb_mix = std::fmax(0.0f, std::fmin(value, 1.0f));
            break;
        // Modulation (chorus/flanger/phaser/vibrato)
        case 0xD7u: // Chorus rate
        case 0xDAu: // Flanger rate
        case 0xDDu: // Phaser rate
        case 0xECu: // Vibrato rate
            lfo_rate = std::fmax(0.01f, std::fmin(value, 50.0f));
            break;
        case 0xD8u: // Chorus depth
        case 0xDBu: // Flanger depth
        case 0xDEu: // Phaser depth
        case 0xEDu: // Vibrato depth
            lfo_depth = std::fmax(0.0f, std::fmin(value, 1.0f));
            break;
        case 0xD9u: // Chorus mix
            mix = std::fmax(0.0f, std::fmin(value, 1.0f));
            break;
        case 0xDCu: // Flanger feedback
            mod_feedback = std::fmax(0.0f, std::fmin(value, 0.99f));
            break;
        case 0xDFu: // Phaser stages
            phaser_stages = std::max(2, std::min(static_cast<int>(value), MAX_PHASER_STAGES));
            break;
        // Pitch shift
        case 0xF0u:
            pitch_semitones = std::fmax(-24.0f, std::fmin(value, 24.0f));
            break;
        case 0xF1u:
            pitch_mix = std::fmax(0.0f, std::fmin(value, 1.0f));
            break;
        }
    }

    void reset() override
    {
        std::memset(delay_buf, 0, sizeof(delay_buf));
        write_pos = 0;
        lfo_phase = 0.0f;
        reverb_lp = 0.0f;
        for (int i = 0; i < NUM_COMBS; i++)
        {
            std::memset(comb_bufs[i], 0, sizeof(comb_bufs[i]));
            comb_pos[i] = 0;
        }
        for (int i = 0; i < NUM_APS; i++)
        {
            std::memset(ap_bufs[i], 0, sizeof(ap_bufs[i]));
            ap_pos[i] = 0;
        }
        std::memset(phaser_ap, 0, sizeof(phaser_ap));
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
        case SIMPLE_DELAY:
            process_delay(in, out, num_samples);
            break;
        case REVERB:
            process_reverb(in, out, num_samples);
            break;
        case CHORUS:
            process_chorus(in, out, num_samples, 5.0f, 20.0f);
            break;
        case FLANGER:
            process_chorus(in, out, num_samples, 1.0f, 7.0f);
            break;
        case PHASER:
            process_phaser(in, out, num_samples);
            break;
        case VIBRATO:
            process_vibrato(in, out, num_samples);
            break;
        case PITCH_SHIFT:
            process_pitch_shift(in, out, num_samples);
            break;
        default:
            std::memcpy(out, in, num_samples * sizeof(float));
            break;
        }
    }

private:
    void update_reverb_params()
    {
        float scale = reverb_size * 0.28f + 0.7f;
        comb_fb = scale;
    }

    void process_delay(const float *in, float *out, uint32_t n)
    {
        int delay_samples = std::max(1, std::min(
                                            static_cast<int>(delay_time_ms * 0.001f * sample_rate),
                                            MAX_DELAY_LEN - 1));
        for (uint32_t i = 0; i < n; ++i)
        {
            int read_pos = (write_pos - delay_samples + MAX_DELAY_LEN) % MAX_DELAY_LEN;
            float delayed = delay_buf[read_pos];
            float input = in[i] + delayed * feedback;
            // NaN/Inf scrub: a single poisoned sample would otherwise
            // recirculate through the delay line forever via the
            // feedback term and never decay. Replace with silence so
            // the line recovers within `delay_samples` samples.
            if (!std::isfinite(input))
                input = 0.0f;
            delay_buf[write_pos] = input;
            write_pos = (write_pos + 1) % MAX_DELAY_LEN;
            float y = in[i] * (1.0f - mix) + delayed * mix;
            if (!std::isfinite(y))
                y = 0.0f;
            out[i] = y;
        }
    }

    void process_reverb(const float *in, float *out, uint32_t n)
    {
        for (uint32_t i = 0; i < n; ++i)
        {
            float input = in[i];
            float comb_sum = 0.0f;

            // Parallel comb filters
            for (int c = 0; c < NUM_COMBS; c++)
            {
                float rd = comb_bufs[c][comb_pos[c]];
                // Low-pass damping
                reverb_lp = rd * (1.0f - reverb_damp) + reverb_lp * reverb_damp;
                float feedback_in = input + reverb_lp * comb_fb;
                if (!std::isfinite(feedback_in))
                {
                    feedback_in = 0.0f;
                    reverb_lp = 0.0f;
                }
                comb_bufs[c][comb_pos[c]] = feedback_in;
                comb_pos[c] = (comb_pos[c] + 1) % comb_lens[c];
                comb_sum += rd;
            }
            comb_sum *= 0.25f;

            // Series all-pass filters
            float ap_out = comb_sum;
            for (int a = 0; a < NUM_APS; a++)
            {
                float buf_val = ap_bufs[a][ap_pos[a]];
                float tmp = -ap_out * ap_g + buf_val;
                float store = ap_out + buf_val * ap_g;
                if (!std::isfinite(store) || !std::isfinite(tmp))
                {
                    store = 0.0f;
                    tmp = 0.0f;
                }
                ap_bufs[a][ap_pos[a]] = store;
                ap_pos[a] = (ap_pos[a] + 1) % ap_lens[a];
                ap_out = tmp;
            }

            float y = in[i] * (1.0f - reverb_mix) + ap_out * reverb_mix;
            if (!std::isfinite(y))
                y = 0.0f;
            out[i] = y;
        }
    }

    void process_chorus(const float *in, float *out, uint32_t n,
                        float base_ms, float sweep_ms)
    {
        float lfo_inc = TWO_PI * lfo_rate / sample_rate;
        for (uint32_t i = 0; i < n; ++i)
        {
            float lfo = 0.5f + 0.5f * std::sin(lfo_phase);
            lfo_phase += lfo_inc;
            if (lfo_phase > TWO_PI)
                lfo_phase -= TWO_PI;

            float delay_ms = base_ms + lfo * lfo_depth * sweep_ms;
            float delay_samps = delay_ms * 0.001f * sample_rate;
            int d0 = static_cast<int>(delay_samps);
            float frac = delay_samps - d0;
            if (d0 < 1)
                d0 = 1;
            if (d0 >= MAX_DELAY_LEN - 1)
                d0 = MAX_DELAY_LEN - 2;

            int r0 = (write_pos - d0 + MAX_DELAY_LEN) % MAX_DELAY_LEN;
            int r1 = (r0 - 1 + MAX_DELAY_LEN) % MAX_DELAY_LEN;
            float delayed = delay_buf[r0] * (1.0f - frac) + delay_buf[r1] * frac;

            float input = in[i] + delayed * mod_feedback;
            delay_buf[write_pos] = input;
            write_pos = (write_pos + 1) % MAX_DELAY_LEN;

            out[i] = in[i] * (1.0f - mix) + delayed * mix;
        }
    }

    void process_phaser(const float *in, float *out, uint32_t n)
    {
        float lfo_inc = TWO_PI * lfo_rate / sample_rate;
        for (uint32_t i = 0; i < n; ++i)
        {
            float lfo = 0.5f + 0.5f * std::sin(lfo_phase);
            lfo_phase += lfo_inc;
            if (lfo_phase > TWO_PI)
                lfo_phase -= TWO_PI;

            // Map LFO to allpass frequency range
            float freq = 200.0f + lfo * lfo_depth * 5000.0f;
            float w = TWO_PI * freq / sample_rate;
            float coeff = (1.0f - std::tan(w * 0.5f)) / (1.0f + std::tan(w * 0.5f));

            float x = in[i];
            float y = x;
            for (int s = 0; s < phaser_stages; ++s)
            {
                float ap_val = coeff * (y - phaser_ap[s]) + y;
                // swap for next iteration
                float tmp = y;
                y = phaser_ap[s] + coeff * (tmp - ap_val);
                y = ap_val;
                phaser_ap[s] = ap_val;
            }
            out[i] = (x + y) * 0.5f;
        }
    }

    void process_vibrato(const float *in, float *out, uint32_t n)
    {
        float lfo_inc = TWO_PI * lfo_rate / sample_rate;
        float max_delay_ms = 7.0f;
        for (uint32_t i = 0; i < n; ++i)
        {
            float lfo = 0.5f + 0.5f * std::sin(lfo_phase);
            lfo_phase += lfo_inc;
            if (lfo_phase > TWO_PI)
                lfo_phase -= TWO_PI;

            float delay_samps = lfo * lfo_depth * max_delay_ms * 0.001f * sample_rate;
            int d0 = static_cast<int>(delay_samps);
            float frac = delay_samps - d0;
            if (d0 < 1)
                d0 = 1;
            if (d0 >= MAX_DELAY_LEN - 1)
                d0 = MAX_DELAY_LEN - 2;

            delay_buf[write_pos] = in[i];
            write_pos = (write_pos + 1) % MAX_DELAY_LEN;

            int r0 = (write_pos - d0 + MAX_DELAY_LEN) % MAX_DELAY_LEN;
            int r1 = (r0 - 1 + MAX_DELAY_LEN) % MAX_DELAY_LEN;
            out[i] = delay_buf[r0] * (1.0f - frac) + delay_buf[r1] * frac;
        }
    }

    void process_pitch_shift(const float *in, float *out, uint32_t n)
    {
        // Simple pitch shift via variable-rate delay line
        float ratio = std::pow(2.0f, pitch_semitones / 12.0f);
        for (uint32_t i = 0; i < n; ++i)
        {
            delay_buf[write_pos] = in[i];
            write_pos = (write_pos + 1) % MAX_DELAY_LEN;

            // Read with fractional offset
            lfo_phase += ratio;
            while (lfo_phase >= MAX_DELAY_LEN)
                lfo_phase -= MAX_DELAY_LEN;
            while (lfo_phase < 0)
                lfo_phase += MAX_DELAY_LEN;

            int r0 = static_cast<int>(lfo_phase);
            float frac = lfo_phase - r0;
            int r1 = (r0 + 1) % MAX_DELAY_LEN;
            float shifted = delay_buf[r0] * (1.0f - frac) + delay_buf[r1] * frac;
            out[i] = in[i] * (1.0f - pitch_mix) + shifted * pitch_mix;
        }
    }
};
