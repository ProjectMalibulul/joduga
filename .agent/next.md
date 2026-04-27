# Loop 27 candidate

**Audit `cpp/include/nodes/delay.h` and `cpp/include/nodes/effects.h`.**

Apply the same checklist that surfaced the priority-1 bugs in loops 23-26:
1. Audio-thread allocation in set_param (vector::resize, vector::assign on
   delay buffers reactive to time-param changes).
2. NaN/Inf state poisoning in any feedback-laden node.
3. Unguarded `static_cast<int>(value)` on subtype/mode params.
4. Single-step phase/buffer wraps that fail under unclamped param values.

Per-file priorities by likely blast radius:
- delay.h: feedback-loop NaN poisoning (highest).
- effects.h: subtype-driven dispatch (chorus, flanger, distortion all
  with their own state — likely a parade of small bugs).

## Backup loops
- Filter resonance-Q ceiling per-mode (loop 25 follow-up — coefficient stability check).
- Carrier-phase `if`-wrap in oscillator.h (defense in depth — currently safe).
- Rate-limit `[midi]` queue-full log (loop 19 follow-up).
- Enum-keyed `BuiltinTemplate` for compile-time-checked catalog lookups.
- End-to-end test for loop-19 vel=0 fix through the C++ engine.
- Audit `audio_engine_destroy` for status_register UAF window.
