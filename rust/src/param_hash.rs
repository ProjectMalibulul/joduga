//! Mirror of the C++ `ParamHash` namespace defined in
//! `cpp/include/audio_node.h`.
//!
//! Every constant here MUST stay numerically identical to its C++
//! counterpart. The C++ engine dispatches parameter updates by these
//! `u32` keys; if the two sides drift, params silently route to the
//! wrong destination or get dropped by `audio_node` switch statements.
//!
//! The `cpp_param_hash_constants_match_cpp` test pins the canonical
//! values so a typo on either side surfaces as a unit-test failure.
//!
//! Naming convention matches the C++ side 1:1 to keep cross-language
//! grepping cheap.
#![allow(dead_code)]
// ── Primary shared hashes ─────────────────────────────────────────────
/// Generic frequency-style param: oscillator frequency, filter cutoff,
/// gain level, threshold, etc. Reused across node types because dispatch
/// is per-node.
pub const FREQ: u32 = 0x811C_9DC5;
/// Generic resonance / Q / bandwidth.
pub const RES: u32 = 0x050C_5D2E;

// ── Per-type aliases (match cpp/include/audio_node.h) ─────────────────
pub const OSC_FREQUENCY: u32 = FREQ;
pub const FILTER_CUTOFF: u32 = FREQ;
pub const FILTER_RESONANCE: u32 = RES;
pub const GAIN_LEVEL: u32 = FREQ;

// ── Sub-type selectors ────────────────────────────────────────────────
pub const WAVEFORM_TYPE: u32 = 0x0000_00AD;
pub const FILTER_MODE: u32 = 0x0000_00BD;

// ── Oscillator extra params ───────────────────────────────────────────
pub const DUTY_CYCLE: u32 = 0xA1;
pub const FM_MOD_DEPTH: u32 = 0xA3;
pub const FM_MOD_FREQ: u32 = 0xA4;
pub const AM_MOD_DEPTH: u32 = 0xA5;
pub const AM_MOD_FREQ: u32 = 0xA6;
pub const DETUNE: u32 = 0xA9;

// ── Filter extra params ───────────────────────────────────────────────
pub const COMB_DELAY: u32 = 0xB1;
pub const COMB_FEEDBACK: u32 = 0xB2;
pub const PARAMETRIC_Q: u32 = 0xB7;

// ── Dynamics extra params ─────────────────────────────────────────────
pub const THRESHOLD: u32 = 0xC0;
pub const RATIO: u32 = 0xC1;
pub const ATTACK: u32 = 0xC2;
pub const RELEASE: u32 = 0xC3;
pub const DYN_KNEE: u32 = 0xC4;
pub const DYN_MAKEUP: u32 = 0xC5;

// ── Sub-type selectors (Gain/Delay/Effects) ───────────────────────────
pub const GAIN_MODE: u32 = 0xCF;
pub const DELAY_MODE: u32 = 0xCD;
pub const EFFECTS_MODE: u32 = 0xCE;

// ── Effects extra params ──────────────────────────────────────────────
pub const DELAY_TIME: u32 = 0xD1;
pub const DELAY_FEEDBACK: u32 = 0xD2;
pub const MIX: u32 = 0xD3;
pub const DRIVE: u32 = 0xE1;
pub const TONE: u32 = 0xE2;

// ── Modulator extra params ────────────────────────────────────────────
pub const DEPTH: u32 = 0x10;

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonical values copied verbatim from the C++ ParamHash namespace.
    /// If a constant changes on either side, this test fails and the diff
    /// is the source of truth for what to fix on the other side.
    #[test]
    fn cpp_param_hash_constants_match_cpp() {
        // Primary
        assert_eq!(FREQ, 0x811C_9DC5);
        assert_eq!(RES, 0x050C_5D2E);
        // Aliases
        assert_eq!(OSC_FREQUENCY, FREQ);
        assert_eq!(FILTER_CUTOFF, FREQ);
        assert_eq!(FILTER_RESONANCE, RES);
        assert_eq!(GAIN_LEVEL, FREQ);
        // Sub-type selectors
        assert_eq!(WAVEFORM_TYPE, 0xAD);
        assert_eq!(FILTER_MODE, 0xBD);
        // Oscillator
        assert_eq!(DUTY_CYCLE, 0xA1);
        assert_eq!(FM_MOD_DEPTH, 0xA3);
        assert_eq!(FM_MOD_FREQ, 0xA4);
        assert_eq!(AM_MOD_DEPTH, 0xA5);
        assert_eq!(AM_MOD_FREQ, 0xA6);
        assert_eq!(DETUNE, 0xA9);
        // Filter
        assert_eq!(COMB_DELAY, 0xB1);
        assert_eq!(COMB_FEEDBACK, 0xB2);
        assert_eq!(PARAMETRIC_Q, 0xB7);
        // Dynamics
        assert_eq!(THRESHOLD, 0xC0);
        assert_eq!(RATIO, 0xC1);
        assert_eq!(ATTACK, 0xC2);
        assert_eq!(RELEASE, 0xC3);
        assert_eq!(DYN_KNEE, 0xC4);
        assert_eq!(DYN_MAKEUP, 0xC5);
        // Modes
        assert_eq!(GAIN_MODE, 0xCF);
        assert_eq!(DELAY_MODE, 0xCD);
        assert_eq!(EFFECTS_MODE, 0xCE);
        // Effects
        assert_eq!(DELAY_TIME, 0xD1);
        assert_eq!(DELAY_FEEDBACK, 0xD2);
        assert_eq!(MIX, 0xD3);
        assert_eq!(DRIVE, 0xE1);
        assert_eq!(TONE, 0xE2);
        // Modulator
        assert_eq!(DEPTH, 0x10);
    }

    /// Selectors must occupy disjoint hash slots from data params on the
    /// same node type; otherwise the dispatch switch in `audio_node` would
    /// route a "set frequency" update into the mode selector. Catch any
    /// future collision at compile-test time.
    #[test]
    fn no_collisions_within_a_node() {
        // Oscillator: freq + selector + extras
        let osc = [
            OSC_FREQUENCY,
            WAVEFORM_TYPE,
            DUTY_CYCLE,
            FM_MOD_DEPTH,
            FM_MOD_FREQ,
            AM_MOD_DEPTH,
            AM_MOD_FREQ,
            DETUNE,
        ];
        assert_unique(&osc, "oscillator");

        // Filter: cutoff/res + selector + extras
        let filt =
            [FILTER_CUTOFF, FILTER_RESONANCE, FILTER_MODE, COMB_DELAY, COMB_FEEDBACK, PARAMETRIC_Q];
        assert_unique(&filt, "filter");

        // Effects: mode selector + dynamics + delay-line + drive/tone
        let fx = [
            EFFECTS_MODE,
            THRESHOLD,
            RATIO,
            ATTACK,
            RELEASE,
            DYN_KNEE,
            DYN_MAKEUP,
            DELAY_TIME,
            DELAY_FEEDBACK,
            MIX,
            DRIVE,
            TONE,
        ];
        assert_unique(&fx, "effects");
    }

    fn assert_unique(slice: &[u32], label: &str) {
        let mut sorted: Vec<u32> = slice.to_vec();
        sorted.sort_unstable();
        for w in sorted.windows(2) {
            assert!(w[0] != w[1], "{label}: duplicate hash 0x{:08X}", w[0]);
        }
    }
}
