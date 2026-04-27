# Next loop seed (loop 31)

## Primary target
`cpp/include/nodes/effects.h` `process_overdrive` (around lines
209-230) — the wet path uses raw `tone_lp` instead of the
tone-blended `tone_lp*tone + distorted*(1-tone)` that distortion
uses. At tone=0 the overdrive wet path silences entirely because
`tone_lp += 0*(distorted - tone_lp)` never updates and `tone_lp`
stays at its initial value.

Fix should mirror process_distortion:
```cpp
float shaped = tone_lp * tone + distorted * (1.0f - tone);
out[i] = in[i] * (1.0f - distort_mix) + shaped * distort_mix;
```

## Secondary target  
`cpp/include/nodes/effects.h` `process_widener` — uses `ap_buf`
and `ap_pos` for what is actually a 512-sample delay line, not an
allpass. The mix `out = in*(1-w) + delayed*w` is a comb filter
(periodic notches at multiples of sample_rate/512 ≈ 93.75 Hz),
not a decorrelator. Either:
  (a) rename to `delay_buf`/`delay_pos` to be honest, or
  (b) implement an actual 1st-order allpass for true decorrelation
      (cheap: same DF-II pattern from delay.h phaser fix in loop 28).

(b) is a real fix (improves stereo widening); (a) is just naming
hygiene. Prefer (b) if loop 31 has budget.

## After loop 31
Loop 32: locate output node / soft-clipper code path. It's not in
`cpp/include/nodes/output.h` (file doesn't exist) — likely lives
in `audio_engine.cpp` or the `audio_engine_wrapper.rs` Rust side.
Audit for DC blocker / final clipper NaN handling.
