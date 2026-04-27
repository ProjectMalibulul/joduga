# Loop 24 candidate

**Audit `oscillator.h::SUPER_SAW` for the same accumulator-blowup pattern.**

Symptom: `saw_phases[j] += TWO_PI * frequency * (1.0f + detune_amt) * sample_rate_inv`
where `detune_amt = (j - v/2) * detune * 0.01`. `detune` is *unclamped*
(line 109: `detune = value;`), and `voices` ranges 1-7. With v=7 and j=6
the multiplier is `3 * detune * 0.01`. detune=1000 → factor=31, frequency=20000 →
per-sample increment ≈ 81 rad. Single-step `if (saw_phases[j] > TWO_PI)`
wrap fails the same way the FM/AM wrap did before loop 23.

**Fix**: clamp DETUNE to [0, 1] (it's a 0-1 amount UI control per design.md §oscillator),
and convert the SUPER_SAW wrap from `if` to `while` for defense in depth.

**Test**: smoke test driving SUPER_SAW (waveform=11) with extreme detune,
assert finite + bounded.

## Backup loops
- Rate-limit `[midi]` queue-full log (loop 19 follow-up).
- Enum-keyed `BuiltinTemplate` for compile-time-checked catalog lookups.
- End-to-end test for loop-19 vel=0 fix through the C++ engine.
- Audit `audio_engine_destroy` for status_register UAF window.
- Carrier-phase `if`-wrap (defense in depth — currently safe under 20 kHz clamp).
