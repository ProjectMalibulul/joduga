# Next loop seed (loop 33)

All five primary DSP node types (oscillator, filter, gain, delay, effects, reverb) now have the canonical NaN/Inf defense pattern. Final-stage ring write hard-clamps to ±1.0. Priority-1 silent-corruption surface across the audio path is exhausted to the best of current knowledge.

Re-prioritize to the long-deferred priority-4 logic bug:

**Loop 33 candidate**: cpp/include/nodes/effects.h `process_overdrive` uses raw `tone_lp` instead of the tone-blended `tone_lp*tone + distorted*(1-tone)` form that `process_distortion` uses. At tone=0 the overdrive output is silent (full LP only, no distorted signal). Fix to mirror distortion's blend.

**Backup candidates**:
- Tauri `start_engine` (tauri-ui/src-tauri/src/main.rs:184,196) silently discards `set_param` errors via `let _ = ...`. UI knob updates dropped to queue backpressure are invisible. Surface to UI.
- audio_engine.cpp param-drain loop memory-ordering audit — verify Acquire/Release pairing and that `pending_params` cap is genuinely structural.
- MIDI input bounds re-audit (loop 23 era).
