# Next loop seed (loop 32)

## Primary target
Re-OBSERVE `cpp/src/audio_engine.cpp` for any remaining
RT-discipline holes:
- allocations on the audio thread (vector reserve, push_back,
  string ops, std::function captures)
- syscalls (any iostream, fprintf, mutex lock, sleep)
- the per-node param-update apply path: does
  `apply_pending_params` allocate or take locks?

## Secondary target
`cpp/include/nodes/oscillator.h` `process()` — even after loop 23
clamp the AM/FM modulation scratch path could produce non-finite
intermediates if `phase_inc * sample_rate` integer-overflows at
extreme frequency settings, or if the multiplicative AM stage
applies an NaN modulator for one sample before the loop-26-style
scrub catches up. Add per-sample output isfinite check on osc
output.

## Tertiary target (priority-4 cleanup)
`cpp/include/nodes/effects.h` `process_overdrive` tone-blend bug
(deferred from loops 29 & 30): wet path uses raw `tone_lp`
instead of the tone-blended `tone_lp*tone + distorted*(1-tone)`.
At tone=0 the wet path silences entirely.

## Pattern check
After loop 32 we will have hardened all 6 node types + the engine
ring boundary. The remaining surface area is:
  - the Rust→C++ FFI boundary (validate ParamUpdateCmd before
    enqueue?)
  - the cpal callback in `audio_engine_wrapper.rs` (does it scrub
    again before writing to the cpal buffer? Probably redundant
    after loop 31 but worth checking)
  - the MIDI input path (`midi_input.rs`) — note-on velocity
    could be 0 unexpectedly, or out-of-range note numbers
