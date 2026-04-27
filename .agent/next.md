# Next loop seed (loop 30)

## Primary target
`cpp/include/nodes/effects.h` `process_overdrive` — at tone=0 the
wet path silences entirely because `tone_lp` is the IIR LP state
that never updates when `tone == 0` (the `tone_lp += tone*(d -
tone_lp)` step is multiplied by 0). Distortion handles this
correctly: `shaped = tone_lp*tone + distorted*(1-tone)` so tone=0
means "no LP filter, pass-through distorted". Overdrive should use
the same blend.

```cpp
// current:
out[i] = in[i] * (1.0f - distort_mix) + tone_lp * distort_mix;
// should be:
float shaped = tone_lp * tone + distorted * (1.0f - tone);
out[i] = in[i] * (1.0f - distort_mix) + shaped * distort_mix;
```

This is a priority-4 logic bug: the overdrive effect produces
silence (or near-silence) for users who set tone=0, which is a
reasonable "off the LP" setting.

## Backup target
`cpp/include/nodes/gain.h` audit (218 lines, not yet reviewed) —
expect set_param NaN guard + GAIN clamp + per-sample NaN scrub
on the smoothed gain target if gain smoothing is implemented.

## After loop 30
Loop 31: gain.h audit. Loop 32: re-audit `output.h` (it's actually
inside the audio_engine wrapper, not a standalone node header —
need to locate the soft-clipper/DC-blocker code path).
