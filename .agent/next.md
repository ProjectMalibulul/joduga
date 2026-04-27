# Next loop seed (loop 38)

Boundary surfaces (host stream, MIDI parser) hardened. Outstanding high-value items:

**Loop 38 candidate**: Tauri command `set_param` (tauri-ui/src-tauri/src/main.rs:222+) takes `Mutex::lock()` per UI knob update, contending with start_engine/stop_engine. Replace `Mutex<Option<RunningEngine>>` with `RwLock<Option<RunningEngine>>` so concurrent set_param calls (UI knob storms) don't serialize on each other.

**Backups**:
- shadow_graph::compile output_buffer_offset overflow audit (256 nodes × MAX_OUTPUTS could overflow u32 only at extreme combinations).
- Frontend (tauri-ui/src/store.ts) error-display surface for the structured set_param errors (loops 34, 36, others).
- Tauri command `set_param` does not validate node_id exists in the running engine — silently dropped if user-side stale id leaks through.
- MIDI queue plumbed but never drained in audio_engine.cpp (large feature gap).
