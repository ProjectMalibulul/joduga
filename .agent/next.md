# Loop 26 candidate

**Audit `cpp/include/nodes/reverb.h` for NaN/Inf propagation and unbounded state.**

Reverb networks (Schroeder, FDN, Freeverb) are notorious for unstable
feedback loops when feedback coefficients approach 1.0 or when an
upstream NaN slips into the comb/allpass delay buffers — once poisoned,
the state never decays. After loop 25's filter NaN-recovery scrub, the
reverb is the next-most-load-bearing node likely to suffer the same
class of bug.

Look for:
1. Param clamping on `REVERB_*` (room size, damping, wet, dry, feedback).
2. Output-mode `static_cast<int>(value)` UB on `REVERB_MODE` (same as filter).
3. NaN recovery on delay-line state — does a single poisoned input stay forever?
4. Per-sample explosion guards (soft clip vs. state clip — same bug class as loop 25).

## Backup loops
- Filter resonance-Q ceiling per-mode (loop 25 follow-up — coefficient stability check before commit).
- Carrier-phase `if`-wrap in oscillator.h (defense in depth — currently safe).
- Rate-limit `[midi]` queue-full log (loop 19 follow-up).
- Enum-keyed `BuiltinTemplate` for compile-time-checked catalog lookups.
- End-to-end test for loop-19 vel=0 fix through the C++ engine.
- Audit `audio_engine_destroy` for status_register UAF window.
