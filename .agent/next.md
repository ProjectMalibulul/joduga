# Next loop seed (loop 34)

DSP correctness surface scrubbed. Time to harden the host/middleware boundary.

**Loop 34 candidate**: tauri-ui/src-tauri/src/main.rs `start_engine` Tauri command at lines 184/196 wraps `engine.set_param(...)` with `let _ = ...`, silently discarding errors. The set_param queue is finite (lock-free SPSC); when the UI smashes a knob faster than the audio thread drains it, errors get swallowed and the user sees no feedback. Surface the failure to the UI as a structured warning return so the React store can display "param dropped" diagnostics.

**Backup**:
- audio_engine.cpp param-drain memory-ordering audit.
- midi_input.rs bounds + reconnect race.
- shadow_graph.rs cycle detection: verify Kahn implementation handles disconnected components correctly.
