# Loop 25 candidate

**Audit `cpp/include/nodes/filter.h` for the same priority-1 patterns.**

After loops 17-24 the oscillator is hardened. The next-most-load-bearing
DSP node is the filter (every realistic patch routes through it). Look for:
1. Coefficient computation under extreme cutoff / Q values — does
   `cos`/`tan` get NaN inputs? Are coefficients clamped to a stable
   region of the bilinear transform?
2. State-variable accumulators (z1/z2 for biquad, or integrator state
   for SVF) — do they admit NaN/Inf if a single sample goes wild upstream?
3. Parameter clamping on `CUTOFF`, `RESONANCE`, `FILTER_TYPE`.
4. Out-of-range FILTER_TYPE subtypes — does the switch fall through to
   silence or undefined behavior?

## Backup loops
- Carrier-phase `if`-wrap in oscillator.h (defense in depth — currently safe).
- Rate-limit `[midi]` queue-full log (loop 19 follow-up).
- Enum-keyed `BuiltinTemplate` for compile-time-checked catalog lookups.
- End-to-end test for loop-19 vel=0 fix through the C++ engine.
- Audit `audio_engine_destroy` for status_register UAF window.
