/** Complete node catalog -- 67 templates across 7 categories.
 *  Icons are short text labels (no emoji). Colors follow Zed palette.
 *
 *  engineSubtype is the mode/subtype WITHIN the C++ node class:
 *    Oscillator: SINE=0, SQUARE=1, SAW=2, TRIANGLE=3, WHITE_NOISE=4,
 *      PINK_NOISE=5, BROWN_NOISE=6, FM=7, AM=8, WAVETABLE=9,
 *      SUB=10, SUPER_SAW=11, ADDITIVE=12
 *    Filter: LP=0, HP=1, BP=2, NOTCH=3, AP=4, COMB=5, FORMANT=6,
 *      MOOG=7, SVF=8, PEAK_EQ=9, LOW_SHELF=10, HIGH_SHELF=11,
 *      TILT=12, DC_BLOCK=13, MOVING_AVG=14, CROSSOVER=15,
 *      RESONATOR=16, VOWEL=17
 *    Gain: SIMPLE_GAIN=0, COMPRESSOR=1, LIMITER=2, GATE=3, EXPANDER=4
 *    Delay: SIMPLE_DELAY=0, REVERB=1, CHORUS=2, FLANGER=3, PHASER=4,
 *      VIBRATO=5, PITCH_SHIFT=6
 *    Effects: DISTORTION=0, OVERDRIVE=1, BITCRUSHER=2, RING_MOD=3,
 *      WAVESHAPER=4, TREMOLO=5, STEREO_WIDENER=6 */

import type { NodeTemplate } from "./types";

export const NODE_CATALOG: NodeTemplate[] = [
    // -- Oscillators (14) ---
    {
        name: "Sine Oscillator", category: "Oscillators", icon: "SIN", color: "#d4976c", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 0, params: [
            { name: "Frequency", hash: 0x811C9DC5, min: 20, max: 20000, default: 440, log: true, suffix: "Hz" },
        ]
    },
    {
        name: "Square Wave", category: "Oscillators", icon: "SQR", color: "#d4976c", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 1, params: [
            { name: "Frequency", hash: 0x811C9DC5, min: 20, max: 20000, default: 440, log: true, suffix: "Hz" },
            { name: "Duty Cycle", hash: 0xA1, min: 0.01, max: 0.99, default: 0.5, log: false, suffix: "" },
        ]
    },
    {
        name: "Sawtooth Wave", category: "Oscillators", icon: "SAW", color: "#d4976c", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 2, params: [
            { name: "Frequency", hash: 0x811C9DC5, min: 20, max: 20000, default: 440, log: true, suffix: "Hz" },
        ]
    },
    {
        name: "Triangle Wave", category: "Oscillators", icon: "TRI", color: "#d4976c", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 3, params: [
            { name: "Frequency", hash: 0x811C9DC5, min: 20, max: 20000, default: 440, log: true, suffix: "Hz" },
        ]
    },
    {
        name: "Pulse Wave", category: "Oscillators", icon: "PLS", color: "#d4976c", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 1, params: [
            { name: "Frequency", hash: 0x811C9DC5, min: 20, max: 20000, default: 440, log: true, suffix: "Hz" },
            { name: "Width", hash: 0xA2, min: 0.01, max: 0.99, default: 0.5, log: false, suffix: "" },
        ]
    },
    {
        name: "White Noise", category: "Oscillators", icon: "WN", color: "#d4976c", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 4, params: [
            { name: "Amplitude", hash: 0x811C9DC5, min: 0, max: 1, default: 0.5, log: false, suffix: "" },
        ]
    },
    {
        name: "Pink Noise", category: "Oscillators", icon: "PN", color: "#d4976c", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 5, params: [
            { name: "Amplitude", hash: 0x811C9DC5, min: 0, max: 1, default: 0.5, log: false, suffix: "" },
        ]
    },
    {
        name: "Brown Noise", category: "Oscillators", icon: "BN", color: "#d4976c", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 6, params: [
            { name: "Amplitude", hash: 0x811C9DC5, min: 0, max: 1, default: 0.5, log: false, suffix: "" },
        ]
    },
    {
        name: "FM Oscillator", category: "Oscillators", icon: "FM", color: "#d4976c", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 7, params: [
            { name: "Carrier", hash: 0x811C9DC5, min: 20, max: 20000, default: 440, log: true, suffix: "Hz" },
            { name: "Mod Depth", hash: 0xA3, min: 0, max: 10, default: 1, log: false, suffix: "" },
            { name: "Mod Freq", hash: 0xA4, min: 0.1, max: 1000, default: 5, log: true, suffix: "Hz" },
        ]
    },
    {
        name: "AM Oscillator", category: "Oscillators", icon: "AM", color: "#d4976c", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 8, params: [
            { name: "Carrier", hash: 0x811C9DC5, min: 20, max: 20000, default: 440, log: true, suffix: "Hz" },
            { name: "Mod Depth", hash: 0xA5, min: 0, max: 1, default: 0.5, log: false, suffix: "" },
            { name: "Mod Freq", hash: 0xA6, min: 0.1, max: 100, default: 5, log: true, suffix: "Hz" },
        ]
    },
    {
        name: "Wavetable", category: "Oscillators", icon: "WT", color: "#d4976c", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 9, params: [
            { name: "Frequency", hash: 0x811C9DC5, min: 20, max: 20000, default: 440, log: true, suffix: "Hz" },
            { name: "Position", hash: 0xA7, min: 0, max: 1, default: 0, log: false, suffix: "" },
        ]
    },
    {
        name: "Sub Oscillator", category: "Oscillators", icon: "SUB", color: "#d4976c", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 10, params: [
            { name: "Frequency", hash: 0x811C9DC5, min: 20, max: 20000, default: 110, log: true, suffix: "Hz" },
            { name: "Octave", hash: 0xA8, min: -3, max: 0, default: -1, log: false, suffix: "" },
        ]
    },
    {
        name: "Super Saw", category: "Oscillators", icon: "SS", color: "#d4976c", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 11, params: [
            { name: "Frequency", hash: 0x811C9DC5, min: 20, max: 20000, default: 440, log: true, suffix: "Hz" },
            { name: "Detune", hash: 0xA9, min: 0, max: 1, default: 0.3, log: false, suffix: "" },
            { name: "Voices", hash: 0xAA, min: 1, max: 7, default: 5, log: false, suffix: "" },
        ]
    },
    {
        name: "Additive Synth", category: "Oscillators", icon: "ADD", color: "#d4976c", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 12, params: [
            { name: "Fundamental", hash: 0x811C9DC5, min: 20, max: 5000, default: 220, log: true, suffix: "Hz" },
            { name: "Harmonics", hash: 0xAB, min: 1, max: 32, default: 8, log: false, suffix: "" },
            { name: "Rolloff", hash: 0xAC, min: 0.1, max: 2, default: 1, log: false, suffix: "" },
        ]
    },

    // -- Filters (18) ---
    {
        name: "Low-Pass Filter", category: "Filters", icon: "LPF", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 0, params: [
            { name: "Cutoff", hash: 0x811C9DC5, min: 20, max: 20000, default: 5000, log: true, suffix: "Hz" },
            { name: "Resonance", hash: 0x050C5D2E, min: 0.1, max: 12, default: 0.707, log: false, suffix: "" },
        ]
    },
    {
        name: "High-Pass Filter", category: "Filters", icon: "HPF", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 1, params: [
            { name: "Cutoff", hash: 0x811C9DC5, min: 20, max: 20000, default: 200, log: true, suffix: "Hz" },
            { name: "Resonance", hash: 0x050C5D2E, min: 0.1, max: 12, default: 0.707, log: false, suffix: "" },
        ]
    },
    {
        name: "Band-Pass Filter", category: "Filters", icon: "BPF", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 2, params: [
            { name: "Center", hash: 0x811C9DC5, min: 20, max: 20000, default: 1000, log: true, suffix: "Hz" },
            { name: "Bandwidth", hash: 0x050C5D2E, min: 0.1, max: 12, default: 1, log: false, suffix: "" },
        ]
    },
    {
        name: "Notch Filter", category: "Filters", icon: "NTC", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 3, params: [
            { name: "Center", hash: 0x811C9DC5, min: 20, max: 20000, default: 1000, log: true, suffix: "Hz" },
            { name: "Width", hash: 0x050C5D2E, min: 0.1, max: 12, default: 1, log: false, suffix: "" },
        ]
    },
    {
        name: "All-Pass Filter", category: "Filters", icon: "APF", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 4, params: [
            { name: "Cutoff", hash: 0x811C9DC5, min: 20, max: 20000, default: 1000, log: true, suffix: "Hz" },
        ]
    },
    {
        name: "Comb Filter", category: "Filters", icon: "CMB", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 5, params: [
            { name: "Delay", hash: 0xB1, min: 0.1, max: 50, default: 5, log: false, suffix: "ms" },
            { name: "Feedback", hash: 0xB2, min: 0, max: 0.99, default: 0.7, log: false, suffix: "" },
        ]
    },
    {
        name: "Formant Filter", category: "Filters", icon: "FMT", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 6, params: [
            { name: "Vowel", hash: 0xB3, min: 0, max: 4, default: 0, log: false, suffix: "" },
            { name: "Shift", hash: 0xB4, min: -12, max: 12, default: 0, log: false, suffix: "st" },
        ]
    },
    {
        name: "Moog Ladder", category: "Filters", icon: "MOG", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 7, params: [
            { name: "Cutoff", hash: 0x811C9DC5, min: 20, max: 20000, default: 2000, log: true, suffix: "Hz" },
            { name: "Resonance", hash: 0x050C5D2E, min: 0, max: 4, default: 1, log: false, suffix: "" },
            { name: "Drive", hash: 0xB5, min: 0, max: 5, default: 0, log: false, suffix: "" },
        ]
    },
    {
        name: "State Variable", category: "Filters", icon: "SVF", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 8, params: [
            { name: "Cutoff", hash: 0x811C9DC5, min: 20, max: 20000, default: 3000, log: true, suffix: "Hz" },
            { name: "Resonance", hash: 0x050C5D2E, min: 0.1, max: 10, default: 0.707, log: false, suffix: "" },
            { name: "Mix LP/HP", hash: 0xB6, min: 0, max: 1, default: 0, log: false, suffix: "" },
        ]
    },
    {
        name: "Parametric EQ", category: "Filters", icon: "PEQ", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 9, params: [
            { name: "Frequency", hash: 0x811C9DC5, min: 20, max: 20000, default: 1000, log: true, suffix: "Hz" },
            { name: "Gain dB", hash: 0x050C5D2E, min: -18, max: 18, default: 0, log: false, suffix: "dB" },
            { name: "Q", hash: 0xB7, min: 0.1, max: 20, default: 1, log: false, suffix: "" },
        ]
    },
    {
        name: "Low Shelf EQ", category: "Filters", icon: "LSH", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 10, params: [
            { name: "Frequency", hash: 0x811C9DC5, min: 20, max: 5000, default: 200, log: true, suffix: "Hz" },
            { name: "Gain dB", hash: 0x050C5D2E, min: -18, max: 18, default: 0, log: false, suffix: "dB" },
        ]
    },
    {
        name: "High Shelf EQ", category: "Filters", icon: "HSH", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 11, params: [
            { name: "Frequency", hash: 0x811C9DC5, min: 1000, max: 20000, default: 8000, log: true, suffix: "Hz" },
            { name: "Gain dB", hash: 0x050C5D2E, min: -18, max: 18, default: 0, log: false, suffix: "dB" },
        ]
    },
    {
        name: "Tilt EQ", category: "Filters", icon: "TLT", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 12, params: [
            { name: "Tilt dB", hash: 0x811C9DC5, min: -6, max: 6, default: 0, log: false, suffix: "dB" },
            { name: "Center", hash: 0x050C5D2E, min: 200, max: 5000, default: 1000, log: true, suffix: "Hz" },
        ]
    },
    {
        name: "DC Blocker", category: "Filters", icon: "DCB", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 13, params: [
            { name: "Cutoff", hash: 0x811C9DC5, min: 5, max: 80, default: 20, log: false, suffix: "Hz" },
        ]
    },
    {
        name: "Moving Average", category: "Filters", icon: "MVA", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 14, params: [
            { name: "Window", hash: 0xB8, min: 1, max: 128, default: 8, log: false, suffix: "samp" },
        ]
    },
    {
        name: "Crossover", category: "Filters", icon: "XOV", color: "#73c990", numInputs: 1, numOutputs: 2, engineType: "Filter", engineSubtype: 15, params: [
            { name: "Frequency", hash: 0x811C9DC5, min: 100, max: 10000, default: 1000, log: true, suffix: "Hz" },
            { name: "Order", hash: 0xB9, min: 1, max: 4, default: 2, log: false, suffix: "" },
        ]
    },
    {
        name: "Resonator", category: "Filters", icon: "RES", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 16, params: [
            { name: "Frequency", hash: 0x811C9DC5, min: 50, max: 10000, default: 500, log: true, suffix: "Hz" },
            { name: "Decay", hash: 0x050C5D2E, min: 0.01, max: 5, default: 0.5, log: true, suffix: "s" },
        ]
    },
    {
        name: "Vowel Filter", category: "Filters", icon: "VWL", color: "#73c990", numInputs: 1, numOutputs: 1, engineType: "Filter", engineSubtype: 17, params: [
            { name: "Vowel", hash: 0xBA, min: 0, max: 4, default: 0, log: false, suffix: "" },
            { name: "Q", hash: 0x050C5D2E, min: 0.5, max: 10, default: 2, log: false, suffix: "" },
        ]
    },

    // -- Dynamics (7) ---
    {
        name: "Gain", category: "Dynamics", icon: "GN", color: "#6eb3f7", numInputs: 1, numOutputs: 1, engineType: "Gain", engineSubtype: 0, params: [
            { name: "Level", hash: 0x811C9DC5, min: 0, max: 2, default: 1, log: false, suffix: "" },
        ]
    },
    {
        name: "Attenuator", category: "Dynamics", icon: "ATT", color: "#6eb3f7", numInputs: 1, numOutputs: 1, engineType: "Gain", engineSubtype: 0, params: [
            { name: "Amount", hash: 0x811C9DC5, min: 0, max: 1, default: 0.5, log: false, suffix: "" },
        ]
    },
    {
        name: "VCA", category: "Dynamics", icon: "VCA", color: "#6eb3f7", numInputs: 1, numOutputs: 1, engineType: "Gain", engineSubtype: 0, params: [
            { name: "Level", hash: 0x811C9DC5, min: 0, max: 2, default: 1, log: false, suffix: "" },
        ]
    },
    {
        name: "Compressor", category: "Dynamics", icon: "CMP", color: "#6eb3f7", numInputs: 1, numOutputs: 1, engineType: "Gain", engineSubtype: 1, params: [
            { name: "Threshold", hash: 0xC0, min: -60, max: 0, default: -20, log: false, suffix: "dB" },
            { name: "Ratio", hash: 0xC1, min: 1, max: 20, default: 4, log: false, suffix: ":1" },
            { name: "Attack", hash: 0xC2, min: 0.1, max: 100, default: 10, log: true, suffix: "ms" },
            { name: "Release", hash: 0xC3, min: 10, max: 1000, default: 100, log: true, suffix: "ms" },
        ]
    },
    {
        name: "Limiter", category: "Dynamics", icon: "LIM", color: "#6eb3f7", numInputs: 1, numOutputs: 1, engineType: "Gain", engineSubtype: 2, params: [
            { name: "Threshold", hash: 0xC0, min: -20, max: 0, default: -3, log: false, suffix: "dB" },
            { name: "Release", hash: 0xC3, min: 10, max: 500, default: 50, log: true, suffix: "ms" },
        ]
    },
    {
        name: "Gate", category: "Dynamics", icon: "GAT", color: "#6eb3f7", numInputs: 1, numOutputs: 1, engineType: "Gain", engineSubtype: 3, params: [
            { name: "Threshold", hash: 0xC0, min: -80, max: 0, default: -40, log: false, suffix: "dB" },
            { name: "Attack", hash: 0xC2, min: 0.1, max: 50, default: 1, log: true, suffix: "ms" },
            { name: "Release", hash: 0xC3, min: 10, max: 500, default: 50, log: true, suffix: "ms" },
        ]
    },
    {
        name: "Expander", category: "Dynamics", icon: "EXP", color: "#6eb3f7", numInputs: 1, numOutputs: 1, engineType: "Gain", engineSubtype: 4, params: [
            { name: "Threshold", hash: 0xC0, min: -60, max: 0, default: -30, log: false, suffix: "dB" },
            { name: "Ratio", hash: 0xC1, min: 1, max: 10, default: 2, log: false, suffix: ":1" },
        ]
    },

    // -- Effects (14) ---
    {
        name: "Delay", category: "Effects", icon: "DLY", color: "#c485ec", numInputs: 1, numOutputs: 1, engineType: "Delay", engineSubtype: 0, params: [
            { name: "Time", hash: 0xD1, min: 1, max: 2000, default: 250, log: true, suffix: "ms" },
            { name: "Feedback", hash: 0xD2, min: 0, max: 0.99, default: 0.5, log: false, suffix: "" },
            { name: "Mix", hash: 0xD3, min: 0, max: 1, default: 0.5, log: false, suffix: "" },
        ]
    },
    {
        name: "Reverb", category: "Effects", icon: "RVB", color: "#c485ec", numInputs: 1, numOutputs: 1, engineType: "Delay", engineSubtype: 1, params: [
            { name: "Size", hash: 0xD4, min: 0, max: 1, default: 0.6, log: false, suffix: "" },
            { name: "Damping", hash: 0xD5, min: 0, max: 1, default: 0.5, log: false, suffix: "" },
            { name: "Mix", hash: 0xD6, min: 0, max: 1, default: 0.3, log: false, suffix: "" },
        ]
    },
    {
        name: "Chorus", category: "Effects", icon: "CHO", color: "#c485ec", numInputs: 1, numOutputs: 1, engineType: "Delay", engineSubtype: 2, params: [
            { name: "Rate", hash: 0xD7, min: 0.1, max: 10, default: 1, log: false, suffix: "Hz" },
            { name: "Depth", hash: 0xD8, min: 0, max: 1, default: 0.5, log: false, suffix: "" },
            { name: "Mix", hash: 0xD9, min: 0, max: 1, default: 0.5, log: false, suffix: "" },
        ]
    },
    {
        name: "Flanger", category: "Effects", icon: "FLG", color: "#c485ec", numInputs: 1, numOutputs: 1, engineType: "Delay", engineSubtype: 3, params: [
            { name: "Rate", hash: 0xDA, min: 0.05, max: 5, default: 0.5, log: false, suffix: "Hz" },
            { name: "Depth", hash: 0xDB, min: 0, max: 1, default: 0.7, log: false, suffix: "" },
            { name: "Feedback", hash: 0xDC, min: 0, max: 0.99, default: 0.7, log: false, suffix: "" },
        ]
    },
    {
        name: "Phaser", category: "Effects", icon: "PHS", color: "#c485ec", numInputs: 1, numOutputs: 1, engineType: "Delay", engineSubtype: 4, params: [
            { name: "Rate", hash: 0xDD, min: 0.05, max: 5, default: 0.3, log: false, suffix: "Hz" },
            { name: "Depth", hash: 0xDE, min: 0, max: 1, default: 0.6, log: false, suffix: "" },
            { name: "Stages", hash: 0xDF, min: 2, max: 12, default: 4, log: false, suffix: "" },
        ]
    },
    {
        name: "Distortion", category: "Effects", icon: "DST", color: "#c485ec", numInputs: 1, numOutputs: 1, engineType: "Effects", engineSubtype: 0, params: [
            { name: "Drive", hash: 0xE1, min: 0, max: 10, default: 3, log: false, suffix: "" },
            { name: "Tone", hash: 0xE2, min: 0, max: 1, default: 0.5, log: false, suffix: "" },
            { name: "Mix", hash: 0xE3, min: 0, max: 1, default: 1, log: false, suffix: "" },
        ]
    },
    {
        name: "Overdrive", category: "Effects", icon: "OVD", color: "#c485ec", numInputs: 1, numOutputs: 1, engineType: "Effects", engineSubtype: 1, params: [
            { name: "Drive", hash: 0xE4, min: 0, max: 10, default: 2, log: false, suffix: "" },
            { name: "Tone", hash: 0xE5, min: 0, max: 1, default: 0.6, log: false, suffix: "" },
        ]
    },
    {
        name: "Bitcrusher", category: "Effects", icon: "BIT", color: "#c485ec", numInputs: 1, numOutputs: 1, engineType: "Effects", engineSubtype: 2, params: [
            { name: "Bits", hash: 0xE6, min: 1, max: 16, default: 8, log: false, suffix: "" },
            { name: "Downsample", hash: 0xE7, min: 1, max: 64, default: 1, log: false, suffix: "x" },
        ]
    },
    {
        name: "Ring Modulator", category: "Effects", icon: "RNG", color: "#c485ec", numInputs: 1, numOutputs: 1, engineType: "Effects", engineSubtype: 3, params: [
            { name: "Frequency", hash: 0xE8, min: 1, max: 5000, default: 200, log: true, suffix: "Hz" },
            { name: "Mix", hash: 0xE9, min: 0, max: 1, default: 0.5, log: false, suffix: "" },
        ]
    },
    {
        name: "Tremolo", category: "Effects", icon: "TRM", color: "#c485ec", numInputs: 1, numOutputs: 1, engineType: "Effects", engineSubtype: 5, params: [
            { name: "Rate", hash: 0xEA, min: 0.1, max: 20, default: 5, log: false, suffix: "Hz" },
            { name: "Depth", hash: 0xEB, min: 0, max: 1, default: 0.5, log: false, suffix: "" },
        ]
    },
    {
        name: "Vibrato", category: "Effects", icon: "VIB", color: "#c485ec", numInputs: 1, numOutputs: 1, engineType: "Delay", engineSubtype: 5, params: [
            { name: "Rate", hash: 0xEC, min: 0.1, max: 20, default: 5, log: false, suffix: "Hz" },
            { name: "Depth", hash: 0xED, min: 0, max: 1, default: 0.3, log: false, suffix: "" },
        ]
    },
    {
        name: "Waveshaper", category: "Effects", icon: "WS", color: "#c485ec", numInputs: 1, numOutputs: 1, engineType: "Effects", engineSubtype: 4, params: [
            { name: "Amount", hash: 0xEE, min: 0, max: 10, default: 1, log: false, suffix: "" },
            { name: "Symmetry", hash: 0xEF, min: -1, max: 1, default: 0, log: false, suffix: "" },
        ]
    },
    {
        name: "Pitch Shifter", category: "Effects", icon: "PIT", color: "#c485ec", numInputs: 1, numOutputs: 1, engineType: "Delay", engineSubtype: 6, params: [
            { name: "Semitones", hash: 0xF0, min: -24, max: 24, default: 0, log: false, suffix: "st" },
            { name: "Mix", hash: 0xF1, min: 0, max: 1, default: 1, log: false, suffix: "" },
        ]
    },
    {
        name: "Stereo Widener", category: "Effects", icon: "STW", color: "#c485ec", numInputs: 1, numOutputs: 1, engineType: "Effects", engineSubtype: 6, params: [
            { name: "Width", hash: 0xF2, min: 0, max: 2, default: 1, log: false, suffix: "" },
        ]
    },

    // -- Modulators (6) ---
    {
        name: "LFO Sine", category: "Modulators", icon: "LFS", color: "#e8b75a", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 0, params: [
            { name: "Rate", hash: 0x811C9DC5, min: 0.01, max: 50, default: 1, log: true, suffix: "Hz" },
            { name: "Depth", hash: 0x10, min: 0, max: 1, default: 1, log: false, suffix: "" },
        ]
    },
    {
        name: "LFO Square", category: "Modulators", icon: "LFQ", color: "#e8b75a", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 1, params: [
            { name: "Rate", hash: 0x811C9DC5, min: 0.01, max: 50, default: 1, log: true, suffix: "Hz" },
            { name: "Depth", hash: 0x11, min: 0, max: 1, default: 1, log: false, suffix: "" },
        ]
    },
    {
        name: "LFO Triangle", category: "Modulators", icon: "LFT", color: "#e8b75a", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 3, params: [
            { name: "Rate", hash: 0x811C9DC5, min: 0.01, max: 50, default: 1, log: true, suffix: "Hz" },
            { name: "Depth", hash: 0x12, min: 0, max: 1, default: 1, log: false, suffix: "" },
        ]
    },
    {
        name: "LFO Sample & Hold", category: "Modulators", icon: "S+H", color: "#e8b75a", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 4, params: [
            { name: "Rate", hash: 0x811C9DC5, min: 0.01, max: 50, default: 2, log: true, suffix: "Hz" },
            { name: "Depth", hash: 0x13, min: 0, max: 1, default: 1, log: false, suffix: "" },
        ]
    },
    {
        name: "ADSR Envelope", category: "Modulators", icon: "ENV", color: "#e8b75a", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 0, params: [
            { name: "Attack", hash: 0x14, min: 0.001, max: 5, default: 0.01, log: true, suffix: "s" },
            { name: "Decay", hash: 0x15, min: 0.001, max: 5, default: 0.1, log: true, suffix: "s" },
            { name: "Sustain", hash: 0x16, min: 0, max: 1, default: 0.7, log: false, suffix: "" },
            { name: "Release", hash: 0x17, min: 0.001, max: 10, default: 0.3, log: true, suffix: "s" },
        ]
    },
    {
        name: "AR Envelope", category: "Modulators", icon: "AR", color: "#e8b75a", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 0, params: [
            { name: "Attack", hash: 0x18, min: 0.001, max: 5, default: 0.01, log: true, suffix: "s" },
            { name: "Release", hash: 0x19, min: 0.001, max: 10, default: 0.3, log: true, suffix: "s" },
        ]
    },

    // -- Utility (7) ---
    {
        name: "Mixer 2-Ch", category: "Utility", icon: "MIX", color: "#a0a4ab", numInputs: 2, numOutputs: 1, engineType: "Gain", engineSubtype: 0, params: [
            { name: "Ch A", hash: 0x20, min: 0, max: 2, default: 1, log: false, suffix: "" },
            { name: "Ch B", hash: 0x21, min: 0, max: 2, default: 1, log: false, suffix: "" },
        ]
    },
    {
        name: "Crossfade", category: "Utility", icon: "XFD", color: "#a0a4ab", numInputs: 2, numOutputs: 1, engineType: "Gain", engineSubtype: 0, params: [
            { name: "Mix", hash: 0x22, min: 0, max: 1, default: 0.5, log: false, suffix: "" },
        ]
    },
    {
        name: "Constant", category: "Utility", icon: "CST", color: "#a0a4ab", numInputs: 0, numOutputs: 1, engineType: "Oscillator", engineSubtype: 0, params: [
            { name: "Value", hash: 0x811C9DC5, min: 0, max: 10, default: 1, log: false, suffix: "" },
        ]
    },
    {
        name: "DC Offset", category: "Utility", icon: "DCO", color: "#a0a4ab", numInputs: 1, numOutputs: 1, engineType: "Gain", engineSubtype: 0, params: [
            { name: "Offset", hash: 0x23, min: -1, max: 1, default: 0, log: false, suffix: "" },
        ]
    },
    { name: "Inverter", category: "Utility", icon: "INV", color: "#a0a4ab", numInputs: 1, numOutputs: 1, engineType: "Gain", engineSubtype: 0, params: [] },
    { name: "Splitter", category: "Utility", icon: "SPL", color: "#a0a4ab", numInputs: 1, numOutputs: 2, engineType: "Gain", engineSubtype: 0, params: [] },

    // -- Output (1) ---
    { name: "Speaker Output", category: "Output", icon: "OUT", color: "#c8c8c8", numInputs: 1, numOutputs: 0, engineType: "Output", engineSubtype: 0, params: [] },
];

/** All unique category names in catalog order. */
export const CATEGORIES = [...new Set(NODE_CATALOG.map((t) => t.category))];
