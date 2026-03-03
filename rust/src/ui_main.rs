/// Joduga — Visual Node-Graph Audio Synthesizer
///
/// A clean node-graph editor for building audio signal chains.
///
/// Controls:
///   • Left panel: searchable node catalog (click to add)
///   • Canvas: drag nodes to move, right-click for quick-add menu
///   • Green ● = output port, Cyan ● = input port
///   • Click output port, then click input port to connect
///   • Escape cancels in-progress connection
///   • Middle-mouse drag or Shift+drag to pan the canvas
///   • ▶ Start / ⏹ Stop to control audio engine
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use joduga::{
    audio_engine_wrapper::{AudioEngineWrapper, OutputRingBuffer},
    ffi::NodeType,
    shadow_graph::{Edge, Node, ShadowGraph},
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// ── FNV-1a param hashes (must match C++ engine) ────────────────────────────
const H_FREQ: u32 = 0x811C9DC5;
const H_RES: u32 = 0x050C5D2E;

// ── Colors ──────────────────────────────────────────────────────────────────
const BG: egui::Color32 = egui::Color32::from_rgb(18, 18, 24);
const SURFACE: egui::Color32 = egui::Color32::from_rgb(30, 30, 40);
const ACCENT: egui::Color32 = egui::Color32::from_rgb(0, 180, 216);
const GREEN: egui::Color32 = egui::Color32::from_rgb(46, 204, 113);
const RED: egui::Color32 = egui::Color32::from_rgb(231, 76, 60);
const C_OSC: egui::Color32 = egui::Color32::from_rgb(255, 183, 77);
const C_FLT: egui::Color32 = egui::Color32::from_rgb(129, 199, 132);
const C_DYN: egui::Color32 = egui::Color32::from_rgb(100, 181, 246);
const C_FX: egui::Color32 = egui::Color32::from_rgb(186, 104, 200);
const C_MOD: egui::Color32 = egui::Color32::from_rgb(255, 213, 79);
const C_UTL: egui::Color32 = egui::Color32::from_rgb(176, 190, 197);
const C_OUT: egui::Color32 = egui::Color32::from_rgb(200, 200, 200);

const PORT_RADIUS: f32 = 5.0;
const PORT_SIZE: f32 = 12.0;
const NODE_WIDTH: f32 = 230.0;

// ═══════════════════════════════════════════════════════════════════════════
//  Node catalog
// ═══════════════════════════════════════════════════════════════════════════
#[derive(Clone)]
struct ParamDef {
    name: &'static str,
    hash: u32,
    min: f32,
    max: f32,
    default: f32,
    log: bool,
    suffix: &'static str,
}

fn pd(
    name: &'static str,
    hash: u32,
    min: f32,
    max: f32,
    default: f32,
    log: bool,
    suffix: &'static str,
) -> ParamDef {
    ParamDef {
        name,
        hash,
        min,
        max,
        default,
        log,
        suffix,
    }
}

#[derive(Clone)]
struct NodeTemplate {
    name: &'static str,
    category: &'static str,
    icon: &'static str,
    color: egui::Color32,
    num_inputs: usize,
    num_outputs: usize,
    engine_type: NodeType,
    params: Vec<ParamDef>,
}

fn catalog() -> Vec<NodeTemplate> {
    use NodeType::*;
    let t = |name, cat, icon, col, ni, no, et, ps: Vec<ParamDef>| NodeTemplate {
        name,
        category: cat,
        icon,
        color: col,
        num_inputs: ni,
        num_outputs: no,
        engine_type: et,
        params: ps,
    };
    vec![
        // ── Oscillators (14) ─────────────────────────────────────────
        t(
            "Sine Oscillator",
            "Oscillators",
            "🎵",
            C_OSC,
            0,
            1,
            Oscillator,
            vec![pd("Frequency", H_FREQ, 20.0, 20000.0, 440.0, true, "Hz")],
        ),
        t(
            "Square Wave",
            "Oscillators",
            "⬜",
            C_OSC,
            0,
            1,
            Oscillator,
            vec![
                pd("Frequency", H_FREQ, 20.0, 20000.0, 440.0, true, "Hz"),
                pd("Duty Cycle", 0xA1, 0.01, 0.99, 0.5, false, ""),
            ],
        ),
        t(
            "Sawtooth Wave",
            "Oscillators",
            "📐",
            C_OSC,
            0,
            1,
            Oscillator,
            vec![pd("Frequency", H_FREQ, 20.0, 20000.0, 440.0, true, "Hz")],
        ),
        t(
            "Triangle Wave",
            "Oscillators",
            "🔺",
            C_OSC,
            0,
            1,
            Oscillator,
            vec![pd("Frequency", H_FREQ, 20.0, 20000.0, 440.0, true, "Hz")],
        ),
        t(
            "Pulse Wave",
            "Oscillators",
            "⚡",
            C_OSC,
            0,
            1,
            Oscillator,
            vec![
                pd("Frequency", H_FREQ, 20.0, 20000.0, 440.0, true, "Hz"),
                pd("Width", 0xA2, 0.01, 0.99, 0.5, false, ""),
            ],
        ),
        t(
            "White Noise",
            "Oscillators",
            "🌫",
            C_OSC,
            0,
            1,
            Oscillator,
            vec![pd("Amplitude", H_FREQ, 0.0, 1.0, 0.5, false, "")],
        ),
        t(
            "Pink Noise",
            "Oscillators",
            "🩷",
            C_OSC,
            0,
            1,
            Oscillator,
            vec![pd("Amplitude", H_FREQ, 0.0, 1.0, 0.5, false, "")],
        ),
        t(
            "Brown Noise",
            "Oscillators",
            "🟤",
            C_OSC,
            0,
            1,
            Oscillator,
            vec![pd("Amplitude", H_FREQ, 0.0, 1.0, 0.5, false, "")],
        ),
        t(
            "FM Oscillator",
            "Oscillators",
            "📻",
            C_OSC,
            0,
            1,
            Oscillator,
            vec![
                pd("Carrier", H_FREQ, 20.0, 20000.0, 440.0, true, "Hz"),
                pd("Mod Depth", 0xA3, 0.0, 10.0, 1.0, false, ""),
                pd("Mod Freq", 0xA4, 0.1, 1000.0, 5.0, true, "Hz"),
            ],
        ),
        t(
            "AM Oscillator",
            "Oscillators",
            "📡",
            C_OSC,
            0,
            1,
            Oscillator,
            vec![
                pd("Carrier", H_FREQ, 20.0, 20000.0, 440.0, true, "Hz"),
                pd("Mod Depth", 0xA5, 0.0, 1.0, 0.5, false, ""),
                pd("Mod Freq", 0xA6, 0.1, 100.0, 5.0, true, "Hz"),
            ],
        ),
        t(
            "Wavetable",
            "Oscillators",
            "🌊",
            C_OSC,
            0,
            1,
            Oscillator,
            vec![
                pd("Frequency", H_FREQ, 20.0, 20000.0, 440.0, true, "Hz"),
                pd("Position", 0xA7, 0.0, 1.0, 0.0, false, ""),
            ],
        ),
        t(
            "Sub Oscillator",
            "Oscillators",
            "🔉",
            C_OSC,
            0,
            1,
            Oscillator,
            vec![
                pd("Frequency", H_FREQ, 20.0, 20000.0, 110.0, true, "Hz"),
                pd("Octave", 0xA8, -3.0, 0.0, -1.0, false, ""),
            ],
        ),
        t(
            "Super Saw",
            "Oscillators",
            "🪚",
            C_OSC,
            0,
            1,
            Oscillator,
            vec![
                pd("Frequency", H_FREQ, 20.0, 20000.0, 440.0, true, "Hz"),
                pd("Detune", 0xA9, 0.0, 1.0, 0.3, false, ""),
                pd("Voices", 0xAA, 1.0, 7.0, 5.0, false, ""),
            ],
        ),
        t(
            "Additive Synth",
            "Oscillators",
            "➕",
            C_OSC,
            0,
            1,
            Oscillator,
            vec![
                pd("Fundamental", H_FREQ, 20.0, 5000.0, 220.0, true, "Hz"),
                pd("Harmonics", 0xAB, 1.0, 32.0, 8.0, false, ""),
                pd("Rolloff", 0xAC, 0.1, 2.0, 1.0, false, ""),
            ],
        ),
        // ── Filters (18) ────────────────────────────────────────────
        t(
            "Low-Pass Filter",
            "Filters",
            "⬇",
            C_FLT,
            1,
            1,
            Filter,
            vec![
                pd("Cutoff", H_FREQ, 20.0, 20000.0, 5000.0, true, "Hz"),
                pd("Resonance", H_RES, 0.1, 12.0, 0.707, false, ""),
            ],
        ),
        t(
            "High-Pass Filter",
            "Filters",
            "⬆",
            C_FLT,
            1,
            1,
            Filter,
            vec![
                pd("Cutoff", H_FREQ, 20.0, 20000.0, 200.0, true, "Hz"),
                pd("Resonance", H_RES, 0.1, 12.0, 0.707, false, ""),
            ],
        ),
        t(
            "Band-Pass Filter",
            "Filters",
            "↔",
            C_FLT,
            1,
            1,
            Filter,
            vec![
                pd("Center", H_FREQ, 20.0, 20000.0, 1000.0, true, "Hz"),
                pd("Bandwidth", H_RES, 0.1, 12.0, 1.0, false, ""),
            ],
        ),
        t(
            "Notch Filter",
            "Filters",
            "🚫",
            C_FLT,
            1,
            1,
            Filter,
            vec![
                pd("Center", H_FREQ, 20.0, 20000.0, 1000.0, true, "Hz"),
                pd("Width", H_RES, 0.1, 12.0, 1.0, false, ""),
            ],
        ),
        t(
            "All-Pass Filter",
            "Filters",
            "🔄",
            C_FLT,
            1,
            1,
            Filter,
            vec![pd("Cutoff", H_FREQ, 20.0, 20000.0, 1000.0, true, "Hz")],
        ),
        t(
            "Comb Filter",
            "Filters",
            "🪮",
            C_FLT,
            1,
            1,
            Filter,
            vec![
                pd("Delay", 0xB1, 0.1, 50.0, 5.0, false, "ms"),
                pd("Feedback", 0xB2, 0.0, 0.99, 0.7, false, ""),
            ],
        ),
        t(
            "Formant Filter",
            "Filters",
            "🗣",
            C_FLT,
            1,
            1,
            Filter,
            vec![
                pd("Vowel", 0xB3, 0.0, 4.0, 0.0, false, ""),
                pd("Shift", 0xB4, -12.0, 12.0, 0.0, false, "st"),
            ],
        ),
        t(
            "Moog Ladder",
            "Filters",
            "🎛",
            C_FLT,
            1,
            1,
            Filter,
            vec![
                pd("Cutoff", H_FREQ, 20.0, 20000.0, 2000.0, true, "Hz"),
                pd("Resonance", H_RES, 0.0, 4.0, 1.0, false, ""),
                pd("Drive", 0xB5, 0.0, 5.0, 0.0, false, ""),
            ],
        ),
        t(
            "State Variable",
            "Filters",
            "🔀",
            C_FLT,
            1,
            1,
            Filter,
            vec![
                pd("Cutoff", H_FREQ, 20.0, 20000.0, 3000.0, true, "Hz"),
                pd("Resonance", H_RES, 0.1, 10.0, 0.707, false, ""),
                pd("Mix LP/HP", 0xB6, 0.0, 1.0, 0.0, false, ""),
            ],
        ),
        t(
            "Parametric EQ",
            "Filters",
            "📊",
            C_FLT,
            1,
            1,
            Filter,
            vec![
                pd("Frequency", H_FREQ, 20.0, 20000.0, 1000.0, true, "Hz"),
                pd("Gain dB", H_RES, -18.0, 18.0, 0.0, false, "dB"),
                pd("Q", 0xB7, 0.1, 20.0, 1.0, false, ""),
            ],
        ),
        t(
            "Low Shelf EQ",
            "Filters",
            "📉",
            C_FLT,
            1,
            1,
            Filter,
            vec![
                pd("Frequency", H_FREQ, 20.0, 5000.0, 200.0, true, "Hz"),
                pd("Gain dB", H_RES, -18.0, 18.0, 0.0, false, "dB"),
            ],
        ),
        t(
            "High Shelf EQ",
            "Filters",
            "📈",
            C_FLT,
            1,
            1,
            Filter,
            vec![
                pd("Frequency", H_FREQ, 1000.0, 20000.0, 8000.0, true, "Hz"),
                pd("Gain dB", H_RES, -18.0, 18.0, 0.0, false, "dB"),
            ],
        ),
        t(
            "Tilt EQ",
            "Filters",
            "↗",
            C_FLT,
            1,
            1,
            Filter,
            vec![
                pd("Tilt dB", H_FREQ, -6.0, 6.0, 0.0, false, "dB"),
                pd("Center", H_RES, 200.0, 5000.0, 1000.0, true, "Hz"),
            ],
        ),
        t(
            "DC Blocker",
            "Filters",
            "🚿",
            C_FLT,
            1,
            1,
            Filter,
            vec![pd("Cutoff", H_FREQ, 5.0, 80.0, 20.0, false, "Hz")],
        ),
        t(
            "Moving Average",
            "Filters",
            "📏",
            C_FLT,
            1,
            1,
            Filter,
            vec![pd("Window", 0xB8, 1.0, 128.0, 8.0, false, "samp")],
        ),
        t(
            "Crossover",
            "Filters",
            "✂",
            C_FLT,
            1,
            2,
            Filter,
            vec![
                pd("Frequency", H_FREQ, 100.0, 10000.0, 1000.0, true, "Hz"),
                pd("Order", 0xB9, 1.0, 4.0, 2.0, false, ""),
            ],
        ),
        t(
            "Resonator",
            "Filters",
            "🔔",
            C_FLT,
            1,
            1,
            Filter,
            vec![
                pd("Frequency", H_FREQ, 50.0, 10000.0, 500.0, true, "Hz"),
                pd("Decay", H_RES, 0.01, 5.0, 0.5, true, "s"),
            ],
        ),
        t(
            "Vowel Filter",
            "Filters",
            "🅰",
            C_FLT,
            1,
            1,
            Filter,
            vec![
                pd("Vowel", 0xBA, 0.0, 4.0, 0.0, false, ""),
                pd("Q", H_RES, 0.5, 10.0, 2.0, false, ""),
            ],
        ),
        // ── Dynamics (7) ────────────────────────────────────────────
        t(
            "Gain",
            "Dynamics",
            "🔊",
            C_DYN,
            1,
            1,
            Gain,
            vec![pd("Level", H_FREQ, 0.0, 2.0, 1.0, false, "")],
        ),
        t(
            "Attenuator",
            "Dynamics",
            "🔉",
            C_DYN,
            1,
            1,
            Gain,
            vec![pd("Amount", H_FREQ, 0.0, 1.0, 0.5, false, "")],
        ),
        t(
            "VCA",
            "Dynamics",
            "🎚",
            C_DYN,
            1,
            1,
            Gain,
            vec![pd("Level", H_FREQ, 0.0, 2.0, 1.0, false, "")],
        ),
        t(
            "Compressor",
            "Dynamics",
            "🗜",
            C_DYN,
            1,
            1,
            Gain,
            vec![
                pd("Threshold", H_FREQ, -60.0, 0.0, -20.0, false, "dB"),
                pd("Ratio", 0xC1, 1.0, 20.0, 4.0, false, ":1"),
                pd("Attack", 0xC2, 0.1, 100.0, 10.0, true, "ms"),
                pd("Release", 0xC3, 10.0, 1000.0, 100.0, true, "ms"),
            ],
        ),
        t(
            "Limiter",
            "Dynamics",
            "🛑",
            C_DYN,
            1,
            1,
            Gain,
            vec![
                pd("Threshold", H_FREQ, -20.0, 0.0, -3.0, false, "dB"),
                pd("Release", 0xC4, 10.0, 500.0, 50.0, true, "ms"),
            ],
        ),
        t(
            "Gate",
            "Dynamics",
            "🚧",
            C_DYN,
            1,
            1,
            Gain,
            vec![
                pd("Threshold", H_FREQ, -80.0, 0.0, -40.0, false, "dB"),
                pd("Attack", 0xC5, 0.1, 50.0, 1.0, true, "ms"),
                pd("Release", 0xC6, 10.0, 500.0, 50.0, true, "ms"),
            ],
        ),
        t(
            "Expander",
            "Dynamics",
            "↕",
            C_DYN,
            1,
            1,
            Gain,
            vec![
                pd("Threshold", H_FREQ, -60.0, 0.0, -30.0, false, "dB"),
                pd("Ratio", 0xC7, 1.0, 10.0, 2.0, false, ":1"),
            ],
        ),
        // ── Effects (14) ────────────────────────────────────────────
        t(
            "Delay",
            "Effects",
            "⏱",
            C_FX,
            1,
            1,
            Filter,
            vec![
                pd("Time", 0xD1, 1.0, 2000.0, 250.0, true, "ms"),
                pd("Feedback", 0xD2, 0.0, 0.99, 0.5, false, ""),
                pd("Mix", 0xD3, 0.0, 1.0, 0.5, false, ""),
            ],
        ),
        t(
            "Reverb",
            "Effects",
            "🏛",
            C_FX,
            1,
            1,
            Filter,
            vec![
                pd("Size", 0xD4, 0.0, 1.0, 0.6, false, ""),
                pd("Damping", 0xD5, 0.0, 1.0, 0.5, false, ""),
                pd("Mix", 0xD6, 0.0, 1.0, 0.3, false, ""),
            ],
        ),
        t(
            "Chorus",
            "Effects",
            "🎭",
            C_FX,
            1,
            1,
            Filter,
            vec![
                pd("Rate", 0xD7, 0.1, 10.0, 1.0, false, "Hz"),
                pd("Depth", 0xD8, 0.0, 1.0, 0.5, false, ""),
                pd("Mix", 0xD9, 0.0, 1.0, 0.5, false, ""),
            ],
        ),
        t(
            "Flanger",
            "Effects",
            "✈",
            C_FX,
            1,
            1,
            Filter,
            vec![
                pd("Rate", 0xDA, 0.05, 5.0, 0.5, false, "Hz"),
                pd("Depth", 0xDB, 0.0, 1.0, 0.7, false, ""),
                pd("Feedback", 0xDC, 0.0, 0.99, 0.7, false, ""),
            ],
        ),
        t(
            "Phaser",
            "Effects",
            "🌀",
            C_FX,
            1,
            1,
            Filter,
            vec![
                pd("Rate", 0xDD, 0.05, 5.0, 0.3, false, "Hz"),
                pd("Depth", 0xDE, 0.0, 1.0, 0.6, false, ""),
                pd("Stages", 0xDF, 2.0, 12.0, 4.0, false, ""),
            ],
        ),
        t(
            "Distortion",
            "Effects",
            "🔥",
            C_FX,
            1,
            1,
            Gain,
            vec![
                pd("Drive", 0xE1, 0.0, 10.0, 3.0, false, ""),
                pd("Tone", 0xE2, 0.0, 1.0, 0.5, false, ""),
                pd("Mix", 0xE3, 0.0, 1.0, 1.0, false, ""),
            ],
        ),
        t(
            "Overdrive",
            "Effects",
            "🎸",
            C_FX,
            1,
            1,
            Gain,
            vec![
                pd("Drive", 0xE4, 0.0, 10.0, 2.0, false, ""),
                pd("Tone", 0xE5, 0.0, 1.0, 0.6, false, ""),
            ],
        ),
        t(
            "Bitcrusher",
            "Effects",
            "👾",
            C_FX,
            1,
            1,
            Gain,
            vec![
                pd("Bits", 0xE6, 1.0, 16.0, 8.0, false, ""),
                pd("Downsample", 0xE7, 1.0, 64.0, 1.0, false, "x"),
            ],
        ),
        t(
            "Ring Modulator",
            "Effects",
            "💍",
            C_FX,
            1,
            1,
            Gain,
            vec![
                pd("Frequency", 0xE8, 1.0, 5000.0, 200.0, true, "Hz"),
                pd("Mix", 0xE9, 0.0, 1.0, 0.5, false, ""),
            ],
        ),
        t(
            "Tremolo",
            "Effects",
            "〰",
            C_FX,
            1,
            1,
            Gain,
            vec![
                pd("Rate", 0xEA, 0.1, 20.0, 5.0, false, "Hz"),
                pd("Depth", 0xEB, 0.0, 1.0, 0.5, false, ""),
            ],
        ),
        t(
            "Vibrato",
            "Effects",
            "🎻",
            C_FX,
            1,
            1,
            Filter,
            vec![
                pd("Rate", 0xEC, 0.1, 20.0, 5.0, false, "Hz"),
                pd("Depth", 0xED, 0.0, 1.0, 0.3, false, ""),
            ],
        ),
        t(
            "Waveshaper",
            "Effects",
            "📈",
            C_FX,
            1,
            1,
            Gain,
            vec![
                pd("Amount", 0xEE, 0.0, 10.0, 1.0, false, ""),
                pd("Symmetry", 0xEF, -1.0, 1.0, 0.0, false, ""),
            ],
        ),
        t(
            "Pitch Shifter",
            "Effects",
            "🎼",
            C_FX,
            1,
            1,
            Filter,
            vec![
                pd("Semitones", 0xF0, -24.0, 24.0, 0.0, false, "st"),
                pd("Mix", 0xF1, 0.0, 1.0, 1.0, false, ""),
            ],
        ),
        t(
            "Stereo Widener",
            "Effects",
            "↔",
            C_FX,
            1,
            1,
            Gain,
            vec![pd("Width", 0xF2, 0.0, 2.0, 1.0, false, "")],
        ),
        // ── Modulators (6) ──────────────────────────────────────────
        t(
            "LFO Sine",
            "Modulators",
            "🔄",
            C_MOD,
            0,
            1,
            Oscillator,
            vec![
                pd("Rate", H_FREQ, 0.01, 50.0, 1.0, true, "Hz"),
                pd("Depth", 0x10, 0.0, 1.0, 1.0, false, ""),
            ],
        ),
        t(
            "LFO Square",
            "Modulators",
            "⬛",
            C_MOD,
            0,
            1,
            Oscillator,
            vec![
                pd("Rate", H_FREQ, 0.01, 50.0, 1.0, true, "Hz"),
                pd("Depth", 0x11, 0.0, 1.0, 1.0, false, ""),
            ],
        ),
        t(
            "LFO Triangle",
            "Modulators",
            "🔺",
            C_MOD,
            0,
            1,
            Oscillator,
            vec![
                pd("Rate", H_FREQ, 0.01, 50.0, 1.0, true, "Hz"),
                pd("Depth", 0x12, 0.0, 1.0, 1.0, false, ""),
            ],
        ),
        t(
            "LFO Sample & Hold",
            "Modulators",
            "🎲",
            C_MOD,
            0,
            1,
            Oscillator,
            vec![
                pd("Rate", H_FREQ, 0.01, 50.0, 2.0, true, "Hz"),
                pd("Depth", 0x13, 0.0, 1.0, 1.0, false, ""),
            ],
        ),
        t(
            "ADSR Envelope",
            "Modulators",
            "📉",
            C_MOD,
            0,
            1,
            Oscillator,
            vec![
                pd("Attack", 0x14, 0.001, 5.0, 0.01, true, "s"),
                pd("Decay", 0x15, 0.001, 5.0, 0.1, true, "s"),
                pd("Sustain", 0x16, 0.0, 1.0, 0.7, false, ""),
                pd("Release", 0x17, 0.001, 10.0, 0.3, true, "s"),
            ],
        ),
        t(
            "AR Envelope",
            "Modulators",
            "📈",
            C_MOD,
            0,
            1,
            Oscillator,
            vec![
                pd("Attack", 0x18, 0.001, 5.0, 0.01, true, "s"),
                pd("Release", 0x19, 0.001, 10.0, 0.3, true, "s"),
            ],
        ),
        // ── Utility (7) ─────────────────────────────────────────────
        t(
            "Mixer 2-Ch",
            "Utility",
            "🎛",
            C_UTL,
            2,
            1,
            Gain,
            vec![
                pd("Ch A", 0x20, 0.0, 2.0, 1.0, false, ""),
                pd("Ch B", 0x21, 0.0, 2.0, 1.0, false, ""),
            ],
        ),
        t(
            "Crossfade",
            "Utility",
            "🔀",
            C_UTL,
            2,
            1,
            Gain,
            vec![pd("Mix", 0x22, 0.0, 1.0, 0.5, false, "")],
        ),
        t(
            "Constant",
            "Utility",
            "🔢",
            C_UTL,
            0,
            1,
            Oscillator,
            vec![pd("Value", H_FREQ, 0.0, 10.0, 1.0, false, "")],
        ),
        t(
            "DC Offset",
            "Utility",
            "➡",
            C_UTL,
            1,
            1,
            Gain,
            vec![pd("Offset", 0x23, -1.0, 1.0, 0.0, false, "")],
        ),
        t("Inverter", "Utility", "🔃", C_UTL, 1, 1, Gain, vec![]),
        t("Splitter", "Utility", "🔱", C_UTL, 1, 2, Gain, vec![]),
        t(
            "Speaker Output",
            "Output",
            "🎧",
            C_OUT,
            1,
            0,
            Output,
            vec![],
        ),
    ]
}

// ═══════════════════════════════════════════════════════════════════════════
//  Graph model
// ═══════════════════════════════════════════════════════════════════════════
#[derive(Clone)]
struct GraphNode {
    id: usize,
    template_idx: usize,
    param_values: Vec<f32>,
    /// Position in *world* coordinates (before pan)
    world_pos: egui::Pos2,
    /// Port positions in *screen* coordinates (updated each frame)
    input_port_screen: Vec<Option<egui::Pos2>>,
    output_port_screen: Vec<Option<egui::Pos2>>,
}

#[derive(Clone)]
struct Wire {
    from_node: usize,
    from_port: usize,
    to_node: usize,
    to_port: usize,
}

/// Deferred UI actions (collected during frame, applied after)
enum UiAction {
    AddNode(usize, egui::Pos2), // template idx, world position
    RemoveNode(usize),
    BeginWire(usize, usize), // from_node, from_port
    FinishWire {
        from_node: usize,
        from_port: usize,
        to_node: usize,
        to_port: usize,
    },
    ParamChanged(u32, u32, f32), // node_id, param_hash, value
}

// ═══════════════════════════════════════════════════════════════════════════
//  Application state
// ═══════════════════════════════════════════════════════════════════════════
struct JodugaApp {
    catalog: Vec<NodeTemplate>,
    nodes: Vec<GraphNode>,
    wires: Vec<Wire>,
    next_id: usize,
    pending_wire: Option<(usize, usize)>, // (from_node_id, from_port)

    // Audio engine
    engine: Option<AudioEngineWrapper>,
    _cpal_stream: Option<cpal::Stream>,
    running: bool,
    status: String,

    // Waveform visualisation
    waveform: Arc<Mutex<Vec<f32>>>,

    // UI state
    search_text: String,
    pan: egui::Vec2, // canvas pan offset
    zoom: f32,       // canvas zoom level (1.0 = 100%)
    show_settings: bool,
    sample_rate: u32,
    buffer_size: u32,
    show_grid: bool,

    // Panel rects (for masking node overflow)
    toolbar_rect: egui::Rect,
    waveform_rect: egui::Rect,
    catalog_rect: egui::Rect,
    settings_rect: Option<egui::Rect>,
}

impl JodugaApp {
    fn new() -> Self {
        let cat = catalog();

        // Find template indices for the default demo chain
        let osc_idx = 0; // Sine Oscillator
        let flt_idx = cat
            .iter()
            .position(|t| t.name == "Low-Pass Filter")
            .unwrap_or(14);
        let gain_idx = cat.iter().position(|t| t.name == "Gain").unwrap_or(32);
        let out_idx = cat
            .iter()
            .position(|t| t.name == "Speaker Output")
            .unwrap_or(cat.len() - 1);

        let make_node = |id: usize, tidx: usize, x: f32, y: f32, cat: &[NodeTemplate]| {
            let tmpl = &cat[tidx];
            GraphNode {
                id,
                template_idx: tidx,
                param_values: tmpl.params.iter().map(|p| p.default).collect(),
                world_pos: egui::pos2(x, y),
                input_port_screen: vec![None; tmpl.num_inputs],
                output_port_screen: vec![None; tmpl.num_outputs],
            }
        };

        let nodes = vec![
            make_node(0, osc_idx, 20.0, 20.0, &cat),
            make_node(1, flt_idx, 290.0, 20.0, &cat),
            make_node(2, gain_idx, 560.0, 20.0, &cat),
            make_node(3, out_idx, 830.0, 20.0, &cat),
        ];
        let wires = vec![
            Wire {
                from_node: 0,
                from_port: 0,
                to_node: 1,
                to_port: 0,
            },
            Wire {
                from_node: 1,
                from_port: 0,
                to_node: 2,
                to_port: 0,
            },
            Wire {
                from_node: 2,
                from_port: 0,
                to_node: 3,
                to_port: 0,
            },
        ];

        Self {
            catalog: cat,
            nodes,
            wires,
            next_id: 4,
            pending_wire: None,
            engine: None,
            _cpal_stream: None,
            running: false,
            status: "Ready — press ▶ Start".into(),
            waveform: Arc::new(Mutex::new(vec![0.0f32; 512])),
            search_text: String::new(),
            pan: egui::Vec2::ZERO,
            zoom: 1.0,
            show_settings: false,
            sample_rate: 48000,
            buffer_size: 256,
            show_grid: true,
            toolbar_rect: egui::Rect::NOTHING,
            waveform_rect: egui::Rect::NOTHING,
            catalog_rect: egui::Rect::NOTHING,
            settings_rect: None,
        }
    }

    fn node_by_id(&self, id: usize) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    fn add_node(&mut self, template_idx: usize, world_pos: egui::Pos2) {
        let tmpl = &self.catalog[template_idx];
        self.nodes.push(GraphNode {
            id: self.next_id,
            template_idx,
            param_values: tmpl.params.iter().map(|p| p.default).collect(),
            world_pos,
            input_port_screen: vec![None; tmpl.num_inputs],
            output_port_screen: vec![None; tmpl.num_outputs],
        });
        self.next_id += 1;
    }

    fn remove_node(&mut self, id: usize) {
        self.wires.retain(|w| w.from_node != id && w.to_node != id);
        self.nodes.retain(|n| n.id != id);
    }

    // ── Audio engine lifecycle ──────────────────────────────────────
    fn start_engine(&mut self) {
        if self.nodes.is_empty() {
            self.status = "Add some nodes first!".into();
            return;
        }

        let max_nodes = self.nodes.len() + 1;
        let mut shadow = ShadowGraph::new(max_nodes as u32);

        for n in &self.nodes {
            let tmpl = &self.catalog[n.template_idx];
            if let Err(e) = shadow.add_node(Node {
                id: n.id as u32,
                node_type: tmpl.engine_type,
                num_inputs: tmpl.num_inputs as u32,
                num_outputs: tmpl.num_outputs as u32,
                parameters: HashMap::new(),
            }) {
                self.status = format!("Graph error: {e}");
                return;
            }
        }

        for w in &self.wires {
            if let Err(e) = shadow.add_edge(Edge {
                from_node_id: w.from_node as u32,
                from_output_idx: w.from_port as u32,
                to_node_id: w.to_node as u32,
                to_input_idx: w.to_port as u32,
            }) {
                self.status = format!("Edge error: {e}");
                return;
            }
        }

        if let Err(e) = shadow.validate() {
            self.status = format!("Validation error: {e}");
            return;
        }

        let (compiled_nodes, compiled_edges, exec_order) = match shadow.compile() {
            Ok(v) => v,
            Err(e) => {
                self.status = format!("Compile error: {e}");
                return;
            }
        };

        let output_id = self
            .nodes
            .iter()
            .find(|n| matches!(self.catalog[n.template_idx].engine_type, NodeType::Output))
            .map(|n| n.id as u32)
            .unwrap_or(0);

        match AudioEngineWrapper::new(
            compiled_nodes,
            compiled_edges,
            exec_order,
            output_id,
            self.sample_rate,
            self.buffer_size,
            0,
        ) {
            Ok(mut eng) => {
                if let Err(e) = eng.start() {
                    self.status = format!("Start error: {e}");
                    return;
                }

                // Push initial parameter values
                for n in &self.nodes {
                    let tmpl = &self.catalog[n.template_idx];
                    for (i, p) in tmpl.params.iter().enumerate() {
                        if i < n.param_values.len() {
                            let _ = eng.set_param(n.id as u32, p.hash, n.param_values[i]);
                        }
                    }
                }

                // Open cpal output stream
                let ring = eng.output_ring();
                let wf_clone = Arc::clone(&self.waveform);
                match open_cpal_stream(ring, wf_clone, self.sample_rate) {
                    Ok(stream) => self._cpal_stream = Some(stream),
                    Err(e) => self.status = format!("Audio output error: {e}"),
                }

                self.engine = Some(eng);
                self.running = true;
                self.status = "▶ Running".into();
            }
            Err(e) => self.status = format!("Init error: {e}"),
        }
    }

    fn stop_engine(&mut self) {
        self._cpal_stream = None;
        if let Some(ref mut eng) = self.engine {
            let _ = eng.stop();
        }
        self.engine = None;
        self.running = false;
        self.status = "⏹ Stopped".into();
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  cpal audio output
// ═══════════════════════════════════════════════════════════════════════════
fn open_cpal_stream(
    ring: Arc<OutputRingBuffer>,
    waveform: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("No audio output device found")?;
    let config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };
    let stream = device
        .build_output_stream(
            &config,
            move |buffer: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let n = ring.read(buffer);
                // Silence any unfilled portion
                for sample in &mut buffer[n..] {
                    *sample = 0.0;
                }
                // Feed waveform visualisation
                if let Ok(mut wf) = waveform.try_lock() {
                    let wf_len = wf.len();
                    let copy_len = buffer.len().min(wf_len);
                    wf.rotate_left(copy_len);
                    wf[wf_len - copy_len..].copy_from_slice(&buffer[..copy_len]);
                }
            },
            |err| eprintln!("cpal error: {err}"),
            None,
        )
        .map_err(|e| format!("{e}"))?;
    stream.play().map_err(|e| format!("{e}"))?;
    Ok(stream)
}

// ═══════════════════════════════════════════════════════════════════════════
//  Drawing helpers
// ═══════════════════════════════════════════════════════════════════════════
fn draw_bezier_wire(
    painter: &egui::Painter,
    from: egui::Pos2,
    to: egui::Pos2,
    color: egui::Color32,
    width: f32,
) {
    let dx = (to.x - from.x).abs().max(50.0) * 0.4;
    let ctrl1 = egui::pos2(from.x + dx, from.y);
    let ctrl2 = egui::pos2(to.x - dx, to.y);
    painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
        [from, ctrl1, ctrl2, to],
        false,
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(width, color),
    ));
}

// ═══════════════════════════════════════════════════════════════════════════
//  Main UI loop
// ═══════════════════════════════════════════════════════════════════════════
impl eframe::App for JodugaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut actions: Vec<UiAction> = Vec::new();

        // ── Top toolbar ─────────────────────────────────────────────
        let toolbar_resp = egui::TopBottomPanel::top("toolbar")
            .frame(
                egui::Frame::new()
                    .fill(SURFACE)
                    .inner_margin(egui::Margin::same(8)),
            )
            .show(ctx, |ui| {
                ui.scope_builder(
                    egui::UiBuilder::new().layer_id(egui::LayerId::new(
                        egui::Order::Tooltip,
                        egui::Id::new("toolbar_layer"),
                    )),
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.heading(egui::RichText::new("🎵 Joduga").strong().color(ACCENT));
                            ui.separator();

                            if self.running {
                                if ui
                                    .button(egui::RichText::new("⏹  Stop").color(RED).strong())
                                    .clicked()
                                {
                                    self.stop_engine();
                                }
                            } else if ui
                                .button(egui::RichText::new("▶  Start").color(GREEN).strong())
                                .clicked()
                            {
                                self.start_engine();
                            }

                            ui.separator();
                            let status_color = if self.running {
                                GREEN
                            } else {
                                egui::Color32::GRAY
                            };
                            ui.label(egui::RichText::new(&self.status).color(status_color));

                            // Right-aligned settings button
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .button(if self.show_settings {
                                            "⚙ Settings ▾"
                                        } else {
                                            "⚙ Settings"
                                        })
                                        .clicked()
                                    {
                                        self.show_settings = !self.show_settings;
                                    }
                                },
                            );

                            if self.pending_wire.is_some() {
                                ui.separator();
                                ui.label(
                                    egui::RichText::new(
                                        "🔗 Click an input port to connect · Esc to cancel",
                                    )
                                    .color(egui::Color32::YELLOW),
                                );
                            }
                        });
                    },
                );
            });
        self.toolbar_rect = toolbar_resp.response.rect;

        // ── Bottom waveform panel ───────────────────────────────────
        let waveform_resp = egui::TopBottomPanel::bottom("waveform_panel")
            .exact_height(100.0)
            .frame(
                egui::Frame::new()
                    .fill(SURFACE)
                    .inner_margin(egui::Margin::same(4)),
            )
            .show(ctx, |ui| {
                ui.scope_builder(
                    egui::UiBuilder::new().layer_id(egui::LayerId::new(
                        egui::Order::Tooltip,
                        egui::Id::new("waveform_layer"),
                    )),
                    |ui| {
                        let wf_data = self.waveform.lock().unwrap().clone();
                        let points: PlotPoints = wf_data
                            .iter()
                            .enumerate()
                            .map(|(i, &v)| [i as f64, v as f64])
                            .collect();
                        Plot::new("waveform_display")
                            .height(85.0)
                            .show_axes(false)
                            .allow_zoom(false)
                            .allow_drag(false)
                            .allow_scroll(false)
                            .include_y(-1.0)
                            .include_y(1.0)
                            .show(ui, |plot_ui| {
                                plot_ui.line(Line::new(points).color(ACCENT));
                            });
                    },
                );
            });
        self.waveform_rect = waveform_resp.response.rect;

        // ── Left panel: node catalog ────────────────────────────────
        let catalog_resp = egui::SidePanel::left("node_catalog")
            .default_width(190.0)
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(24, 24, 32))
                    .inner_margin(egui::Margin::same(8)),
            )
            .show(ctx, |ui| {
                ui.scope_builder(
                    egui::UiBuilder::new().layer_id(egui::LayerId::new(
                        egui::Order::Tooltip,
                        egui::Id::new("catalog_layer"),
                    )),
                    |ui| {
                        ui.heading(egui::RichText::new("Node Catalog").color(ACCENT));
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.label("🔍");
                            ui.text_edit_singleline(&mut self.search_text);
                        });
                        ui.separator();

                        let search_lower = self.search_text.to_lowercase();

                        // Collect unique categories in order
                        let mut categories: Vec<&str> = Vec::new();
                        for tmpl in &self.catalog {
                            if !categories.contains(&tmpl.category) {
                                categories.push(tmpl.category);
                            }
                        }

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            if self.running {
                                ui.disable();
                            }

                            for cat in &categories {
                                let matching: Vec<(usize, &NodeTemplate)> = self
                                    .catalog
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, t)| t.category == *cat)
                                    .filter(|(_, t)| {
                                        search_lower.is_empty()
                                            || t.name.to_lowercase().contains(&search_lower)
                                    })
                                    .collect();

                                if matching.is_empty() {
                                    continue;
                                }

                                let cat_color = matching[0].1.color;
                                egui::CollapsingHeader::new(
                                    egui::RichText::new(*cat).color(cat_color).strong(),
                                )
                                .default_open(true)
                                .show(ui, |ui| {
                                    for (idx, tmpl) in matching {
                                        let label = format!("{} {}", tmpl.icon, tmpl.name);
                                        let tooltip = format!(
                                            "{} inputs, {} outputs",
                                            tmpl.num_inputs, tmpl.num_outputs
                                        );
                                        if ui.button(&label).on_hover_text(tooltip).clicked() {
                                            // Spawn at a reasonable position, offset by node count
                                            let offset = (self.nodes.len() as f32) * 30.0;
                                            let world_pos =
                                                egui::pos2(100.0 + offset, 50.0 + (offset % 300.0));
                                            actions.push(UiAction::AddNode(idx, world_pos));
                                        }
                                    }
                                });
                            }
                        });
                    },
                );
            });
        self.catalog_rect = catalog_resp.response.rect;

        // ── Right panel: settings ───────────────────────────────────
        self.settings_rect = None;
        if self.show_settings {
            let settings_resp = egui::SidePanel::right("settings_panel")
                .default_width(210.0)
                .frame(
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgb(24, 24, 32))
                        .inner_margin(egui::Margin::same(8)),
                )
                .show(ctx, |ui| {
                    ui.scope_builder(
                        egui::UiBuilder::new().layer_id(egui::LayerId::new(
                            egui::Order::Tooltip,
                            egui::Id::new("settings_layer"),
                        )),
                        |ui| {
                            ui.heading(egui::RichText::new("⚙ Settings").color(ACCENT));
                            ui.separator();

                            ui.label(
                                egui::RichText::new("Audio")
                                    .strong()
                                    .color(egui::Color32::LIGHT_GRAY),
                            );
                            ui.horizontal(|ui| {
                                ui.label("Sample Rate:");
                                egui::ComboBox::from_id_salt("sample_rate")
                                    .selected_text(format!("{} Hz", self.sample_rate))
                                    .show_ui(ui, |ui| {
                                        for sr in [22050, 44100, 48000, 96000u32] {
                                            ui.selectable_value(
                                                &mut self.sample_rate,
                                                sr,
                                                format!("{sr} Hz"),
                                            );
                                        }
                                    });
                            });
                            ui.horizontal(|ui| {
                                ui.label("Buffer Size:");
                                egui::ComboBox::from_id_salt("buffer_size")
                                    .selected_text(format!("{}", self.buffer_size))
                                    .show_ui(ui, |ui| {
                                        for bs in [64, 128, 256, 512, 1024u32] {
                                            ui.selectable_value(
                                                &mut self.buffer_size,
                                                bs,
                                                format!("{bs}"),
                                            );
                                        }
                                    });
                            });

                            ui.add_space(10.0);
                            ui.label(
                                egui::RichText::new("Display")
                                    .strong()
                                    .color(egui::Color32::LIGHT_GRAY),
                            );
                            ui.checkbox(&mut self.show_grid, "Show Grid");

                            ui.add_space(10.0);
                            ui.label(
                                egui::RichText::new("Graph Info")
                                    .strong()
                                    .color(egui::Color32::LIGHT_GRAY),
                            );
                            ui.label(format!("Nodes: {}", self.nodes.len()));
                            ui.label(format!("Connections: {}", self.wires.len()));
                            ui.label(format!("Zoom: {:.0}%", self.zoom * 100.0));
                            if ui.small_button("Reset Zoom").clicked() {
                                self.zoom = 1.0;
                            }

                            ui.add_space(10.0);
                            ui.separator();
                            ui.label(
                                egui::RichText::new("Controls")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                            ui.label(
                                egui::RichText::new(
                                    "• Left panel or right-click → add nodes\n\
                         • Click green ● then cyan ● → connect\n\
                         • Middle-drag or Shift+drag → pan\n\
                         • Ctrl+scroll → zoom in/out\n\
                         • Drag title bar → move node\n\
                         • Drag node edge → resize\n\
                         • ✖ or close → remove node\n\
                         • Escape → cancel connection",
                                )
                                .small()
                                .color(egui::Color32::GRAY),
                            );
                        },
                    );
                });
            self.settings_rect = Some(settings_resp.response.rect);
        }

        // ── Central canvas ──────────────────────────────────────────
        let mut saved_canvas_rect =
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(BG))
            .show(ctx, |ui| {
                let canvas_rect = ui.max_rect();
                saved_canvas_rect = canvas_rect;

                // Canvas interaction: pan + right-click menu
                let canvas_resp = ui.interact(
                    canvas_rect,
                    egui::Id::new("canvas_bg"),
                    egui::Sense::click_and_drag(),
                );

                // Pan: middle-mouse drag or shift+drag
                if canvas_resp.dragged_by(egui::PointerButton::Middle)
                    || (canvas_resp.dragged() && ctx.input(|i| i.modifiers.shift))
                {
                    self.pan += canvas_resp.drag_delta();
                }

                // Scroll: Ctrl+scroll = zoom, normal scroll = pan
                // Check pointer position directly (not canvas_resp.hovered()) so
                // zoom/scroll works even when cursor is over a node Window
                let pointer_in_canvas = ctx.input(|i| {
                    i.pointer
                        .hover_pos()
                        .map(|p| canvas_rect.contains(p))
                        .unwrap_or(false)
                });
                if pointer_in_canvas {
                    let ctrl = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
                    let scroll_y = ctx.input(|i| i.smooth_scroll_delta.y);
                    let pinch_zoom = ctx.input(|i| i.zoom_delta());
                    let mouse_pos =
                        ctx.input(|i| i.pointer.hover_pos().unwrap_or(canvas_rect.center()));

                    // Pinch-to-zoom (trackpad)
                    if pinch_zoom != 1.0 {
                        let mwx = (mouse_pos.x - canvas_rect.min.x - self.pan.x) / self.zoom;
                        let mwy = (mouse_pos.y - canvas_rect.min.y - self.pan.y) / self.zoom;
                        self.zoom = (self.zoom * pinch_zoom).clamp(0.15, 5.0);
                        self.pan.x = mouse_pos.x - canvas_rect.min.x - mwx * self.zoom;
                        self.pan.y = mouse_pos.y - canvas_rect.min.y - mwy * self.zoom;
                    } else if ctrl && scroll_y != 0.0 {
                        // Ctrl+scroll zoom
                        let mwx = (mouse_pos.x - canvas_rect.min.x - self.pan.x) / self.zoom;
                        let mwy = (mouse_pos.y - canvas_rect.min.y - self.pan.y) / self.zoom;
                        let zd = if scroll_y > 0.0 { 1.1 } else { 1.0 / 1.1 };
                        self.zoom = (self.zoom * zd).clamp(0.15, 5.0);
                        self.pan.x = mouse_pos.x - canvas_rect.min.x - mwx * self.zoom;
                        self.pan.y = mouse_pos.y - canvas_rect.min.y - mwy * self.zoom;
                    } else {
                        // Regular scroll = pan
                        let scroll = ctx.input(|i| i.smooth_scroll_delta);
                        if scroll != egui::Vec2::ZERO {
                            self.pan += scroll;
                        }
                    }
                }

                // Right-click context menu for quick-adding nodes
                let click_screen_pos = canvas_resp
                    .interact_pointer_pos()
                    .unwrap_or(canvas_rect.center());
                let click_world_pos = egui::pos2(
                    (click_screen_pos.x - canvas_rect.min.x - self.pan.x) / self.zoom,
                    (click_screen_pos.y - canvas_rect.min.y - self.pan.y) / self.zoom,
                );

                canvas_resp.context_menu(|ui| {
                    if self.running {
                        ui.label("Stop the engine to edit the graph.");
                        return;
                    }
                    ui.heading("Add Node");
                    ui.separator();

                    // Quick-access: Output first
                    for (idx, tmpl) in self.catalog.iter().enumerate() {
                        if tmpl.name == "Speaker Output"
                            && ui.button(format!("{} {}", tmpl.icon, tmpl.name)).clicked()
                        {
                            actions.push(UiAction::AddNode(idx, click_world_pos));
                            ui.close_menu();
                        }
                    }
                    ui.separator();

                    // Sub-menus by category
                    for cat_name in [
                        "Oscillators",
                        "Filters",
                        "Dynamics",
                        "Effects",
                        "Modulators",
                        "Utility",
                    ] {
                        ui.menu_button(cat_name, |ui| {
                            for (idx, tmpl) in self.catalog.iter().enumerate() {
                                if tmpl.category == cat_name
                                    && ui.button(format!("{} {}", tmpl.icon, tmpl.name)).clicked()
                                {
                                    actions.push(UiAction::AddNode(idx, click_world_pos));
                                    ui.close_menu();
                                }
                            }
                        });
                    }
                });

                let painter = ui.painter();

                // ── Draw grid ───────────────────────────────────────
                if self.show_grid {
                    let grid_spacing = 40.0f32 * self.zoom;
                    let grid_color = egui::Color32::from_rgba_premultiplied(50, 50, 70, 30);
                    let stroke = egui::Stroke::new(0.5, grid_color);

                    let offset_x = self.pan.x.rem_euclid(grid_spacing);
                    let offset_y = self.pan.y.rem_euclid(grid_spacing);

                    let mut x = canvas_rect.min.x + offset_x;
                    while x <= canvas_rect.max.x {
                        painter.line_segment(
                            [
                                egui::pos2(x, canvas_rect.min.y),
                                egui::pos2(x, canvas_rect.max.y),
                            ],
                            stroke,
                        );
                        x += grid_spacing;
                    }
                    let mut y = canvas_rect.min.y + offset_y;
                    while y <= canvas_rect.max.y {
                        painter.line_segment(
                            [
                                egui::pos2(canvas_rect.min.x, y),
                                egui::pos2(canvas_rect.max.x, y),
                            ],
                            stroke,
                        );
                        y += grid_spacing;
                    }
                }

                // ── Draw established wires ──────────────────────────
                for wire in &self.wires {
                    let from_pos = self
                        .node_by_id(wire.from_node)
                        .and_then(|n| n.output_port_screen.get(wire.from_port).copied().flatten());
                    let to_pos = self
                        .node_by_id(wire.to_node)
                        .and_then(|n| n.input_port_screen.get(wire.to_port).copied().flatten());
                    if let (Some(fp), Some(tp)) = (from_pos, to_pos) {
                        draw_bezier_wire(painter, fp, tp, ACCENT, 2.5);
                    }
                }

                // ── Draw in-progress wire ───────────────────────────
                if let Some((from_id, from_port)) = self.pending_wire {
                    if let Some(from_pos) = self
                        .node_by_id(from_id)
                        .and_then(|n| n.output_port_screen.get(from_port).copied().flatten())
                    {
                        if let Some(mouse_pos) = ctx.pointer_hover_pos() {
                            draw_bezier_wire(
                                painter,
                                from_pos,
                                mouse_pos,
                                egui::Color32::from_rgba_premultiplied(0, 180, 216, 120),
                                2.0,
                            );
                        }
                    }
                }

                // ── Empty canvas hint ───────────────────────────────
                if self.nodes.is_empty() {
                    painter.text(
                        canvas_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "Right-click or use the left panel to add nodes",
                        egui::FontId::proportional(18.0),
                        egui::Color32::GRAY,
                    );
                }
            });

        // ── Render nodes as Windows (draggable, resizable) ────────
        let pending = self.pending_wire;
        let is_running = self.running;
        let cat = &self.catalog;
        let pan = self.pan;
        let zoom = self.zoom;
        let canvas_rect = saved_canvas_rect;

        for node in &mut self.nodes {
            let node_id = node.id;
            let tmpl = &cat[node.template_idx];
            let node_color = tmpl.color;

            // Screen position = canvas origin + world position * zoom + pan
            let screen_pos = egui::pos2(
                canvas_rect.min.x + node.world_pos.x * zoom + pan.x,
                canvas_rect.min.y + node.world_pos.y * zoom + pan.y,
            );

            let mut node_open = true;
            let win_id = egui::Id::new(("node_win", node_id));

            let win_resp = egui::Window::new(
                egui::RichText::new(format!("{} {} #{}", tmpl.icon, tmpl.name, node_id))
                    .strong()
                    .color(node_color),
            )
            .id(win_id)
            .current_pos(screen_pos)
            .default_width(NODE_WIDTH)
            .movable(!is_running)
            .resizable(!is_running)
            .collapsible(false)
            .title_bar(true)
            .constrain_to(canvas_rect)
            .frame(
                egui::Frame::new()
                    .fill(SURFACE)
                    .stroke(egui::Stroke::new(2.0, node_color))
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::same(8)),
            )
            .open(&mut node_open)
            .show(ctx, |ui| {
                // Clip content to canvas area so nodes don't render over panels
                ui.set_clip_rect(ui.clip_rect().intersect(canvas_rect));

                // ── Input ports ─────────────────────────
                for port_idx in 0..tmpl.num_inputs {
                    ui.horizontal(|ui| {
                        let (port_rect, port_resp) = ui.allocate_exact_size(
                            egui::vec2(PORT_SIZE, PORT_SIZE),
                            egui::Sense::click(),
                        );
                        let port_color = if pending.is_some() && port_resp.hovered() {
                            egui::Color32::WHITE
                        } else {
                            ACCENT
                        };
                        ui.painter()
                            .circle_filled(port_rect.center(), PORT_RADIUS, port_color);
                        if pending.is_some() && port_resp.hovered() {
                            ui.painter().circle_stroke(
                                port_rect.center(),
                                PORT_RADIUS + 3.0,
                                egui::Stroke::new(1.5, egui::Color32::WHITE),
                            );
                        }
                        ui.label(egui::RichText::new(format!("In {port_idx}")).small());

                        if port_idx < node.input_port_screen.len() {
                            node.input_port_screen[port_idx] = Some(port_rect.center());
                        }

                        if port_resp.clicked() {
                            if let Some((from_id, from_port)) = pending {
                                if from_id != node_id {
                                    actions.push(UiAction::FinishWire {
                                        from_node: from_id,
                                        from_port,
                                        to_node: node_id,
                                        to_port: port_idx,
                                    });
                                }
                            }
                        }
                    });
                }

                // ── Parameters ──────────────────────────
                for (pi, pdef) in tmpl.params.iter().enumerate() {
                    if pi >= node.param_values.len() {
                        break;
                    }
                    let mut val = node.param_values[pi];
                    let mut slider = egui::Slider::new(&mut val, pdef.min..=pdef.max);
                    if pdef.log {
                        slider = slider.logarithmic(true);
                    }
                    if !pdef.suffix.is_empty() {
                        slider = slider.suffix(format!(" {}", pdef.suffix));
                    }
                    slider = slider.text(pdef.name);
                    if ui.add(slider).changed() {
                        node.param_values[pi] = val;
                        actions.push(UiAction::ParamChanged(node_id as u32, pdef.hash, val));
                    }
                }

                // ── Output ports ────────────────────────
                for port_idx in 0..tmpl.num_outputs {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (port_rect, port_resp) = ui.allocate_exact_size(
                            egui::vec2(PORT_SIZE, PORT_SIZE),
                            egui::Sense::click(),
                        );
                        let port_color = if port_resp.hovered() {
                            egui::Color32::WHITE
                        } else {
                            GREEN
                        };
                        ui.painter()
                            .circle_filled(port_rect.center(), PORT_RADIUS, port_color);
                        ui.label(egui::RichText::new(format!("Out {port_idx}")).small());

                        if port_idx < node.output_port_screen.len() {
                            node.output_port_screen[port_idx] = Some(port_rect.center());
                        }

                        if port_resp.clicked() && pending.is_none() {
                            actions.push(UiAction::BeginWire(node_id, port_idx));
                        }
                    });
                }
            });

            // Track window drag -> update world position
            if let Some(ref resp) = win_resp {
                if !is_running && resp.response.dragged() {
                    let d = resp.response.drag_delta();
                    node.world_pos.x += d.x / zoom;
                    node.world_pos.y += d.y / zoom;
                }
            }

            // Delete node via window close button
            if !node_open && !is_running {
                actions.push(UiAction::RemoveNode(node_id));
            }
        }

        // ── Paint panel masks on Foreground layer ────────────────────
        // This covers any node Window edges that extend into panel areas.
        // Panel content is on Order::Tooltip (above this), panel backgrounds
        // are on Order::Background (below nodes). These masks sit on
        // Order::Foreground which is above node Windows (Order::Middle).
        {
            let mask = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("panel_masks"),
            ));
            let _screen = ctx.screen_rect();

            // Top toolbar mask
            if self.toolbar_rect != egui::Rect::NOTHING {
                mask.rect_filled(self.toolbar_rect, 0.0, SURFACE);
            }
            // Bottom waveform mask
            if self.waveform_rect != egui::Rect::NOTHING {
                mask.rect_filled(self.waveform_rect, 0.0, SURFACE);
            }
            // Left catalog mask
            if self.catalog_rect != egui::Rect::NOTHING {
                mask.rect_filled(self.catalog_rect, 0.0, egui::Color32::from_rgb(24, 24, 32));
            }
            // Right settings mask
            if let Some(sr) = self.settings_rect {
                mask.rect_filled(sr, 0.0, egui::Color32::from_rgb(24, 24, 32));
            }
        }

        // ── Process deferred actions ────────────────────────────────
        let mut wire_completed = false;
        for action in actions {
            match action {
                UiAction::AddNode(template_idx, world_pos) => {
                    self.add_node(template_idx, world_pos);
                }
                UiAction::RemoveNode(id) => {
                    self.remove_node(id);
                }
                UiAction::BeginWire(node_id, port) => {
                    self.pending_wire = Some((node_id, port));
                }
                UiAction::FinishWire {
                    from_node,
                    from_port,
                    to_node,
                    to_port,
                } => {
                    // Remove any existing wire to the same input
                    self.wires
                        .retain(|w| !(w.to_node == to_node && w.to_port == to_port));
                    self.wires.push(Wire {
                        from_node,
                        from_port,
                        to_node,
                        to_port,
                    });
                    self.pending_wire = None;
                    wire_completed = true;
                }
                UiAction::ParamChanged(node_id, hash, value) => {
                    if let Some(ref eng) = self.engine {
                        let _ = eng.set_param(node_id, hash, value);
                    }
                }
            }
        }

        // Cancel pending wire on Escape
        if self.pending_wire.is_some()
            && !wire_completed
            && ctx.input(|i| i.key_pressed(egui::Key::Escape))
        {
            self.pending_wire = None;
        }

        // Continuous repaint while engine is running (for waveform animation)
        if self.running {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Entry point
// ═══════════════════════════════════════════════════════════════════════════
fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("Joduga — Node-Based Audio Synthesizer"),
        ..Default::default()
    };

    eframe::run_native(
        "Joduga",
        options,
        Box::new(|cc| {
            // Apply dark theme
            let mut style = (*cc.egui_ctx.style()).clone();
            style.visuals = egui::Visuals::dark();
            style.visuals.panel_fill = BG;
            style.visuals.window_fill = SURFACE;
            style.spacing.item_spacing = egui::vec2(6.0, 4.0);
            cc.egui_ctx.set_style(style);
            Ok(Box::new(JodugaApp::new()))
        }),
    )
}
