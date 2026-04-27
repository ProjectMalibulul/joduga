# Next loop seed (loop 29)

## Target
`cpp/include/nodes/effects.h` — full audit. Highest-priority
patterns to look for:

1. `EFFECT_MODE = static_cast<int>(value)` on raw param (NaN UB,
   loop 25/27 pattern).
2. Distortion / waveshaper saturation that uses recursion or
   unbounded drive multipliers — input clamp before non-linearity.
3. Bitcrusher sample-rate reducer: counter integer arithmetic must
   not overflow on large rate-reduction factors; output must be
   sampled-and-held correctly.
4. Effects stack typically has `wet * effect(in) + (1-wet) * in` —
   verify wet is clamped [0,1] and the combined result NaN-scrubbed.
5. RT-discipline: any std::vector resize, std::string ops, or
   heap allocation in process()? Pre-allocate in constructor.

## Guideline
Apply same defense-in-depth as loops 25-28:
- early `if (!std::isfinite(value)) return;` at set_param entry
- mode enum casts: cast → range-clamp → assign
- per-sample NaN scrub on stateful feedback paths
- audio-thread allocation: pre-size in constructor, std::fill in setter

## After loop 29
Loop 30 candidates (in priority order):
- gain.h audit (parameter automation safety)
- output.h audit (DC-blocker / soft-clipper state)
- The FFI boundary in `rust/src/audio_engine_wrapper.rs` for
  param-value validation symmetry with the new C++ guards.
