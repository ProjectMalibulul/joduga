# Next loop seed (loop 36)

Audio thread SPSC ordering now matches the Rust side. Moving outward to other queue/concurrency surfaces.

**Loop 36 candidate**: midi_input.rs reconnect/disconnect race. The midir listener pushes events into an SPSC queue; if the user hot-swaps a MIDI device the listener may rebuild while the audio thread is still draining the queue. Audit for races.

**Backups**:
- shadow_graph.rs add_edge cap of 1024 — confirm overflow is detected at add_edge, not just compile.
- Frontend (tauri-ui/src/store.ts) error display for the new structured set_param error from loop 34.
- Tauri command `set_param` (line 222+ of main.rs) uses `Mutex::lock` per knob update — every UI knob movement takes a lock contended with start/stop. Atomic pointer swap could remove that.
