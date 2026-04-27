# Next loop seed (loop 37)

cpal output is now device-agnostic. Going wider.

**Loop 37 candidate**: `set_param` Tauri command (tauri-ui/src-tauri/src/main.rs:222+) takes a Mutex on every UI knob update. The audio engine is already lock-free; the lock here only protects the `Option<RunningEngine>` swap on start/stop. Consider an `RwLock` (read locks for set_param, write for start/stop) or an `ArcSwap`. UI knob storms during playback contend with the audio thread's own start/stop never, but multiple concurrent `set_param` calls all serialize on the same Mutex — preventing batched UI updates.

**Backups**:
- MIDI parser: mask data bytes with `& 0x7F` to defend against malformed devices that set the high bit.
- MIDI queue **is plumbed but never drained** by audio_engine.cpp — feature gap, large change.
- Frontend (tauri-ui/src/store.ts) error-display surface for the structured set_param error from loop 34.
