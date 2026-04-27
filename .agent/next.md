# Next loop seed (loop 40)

Boundary validations for set_param now cover node existence + param queue backpressure + isfinite-at-DSP-entry. The Tauri host is robust.

**Loop 40 candidate**: `start_engine` does not validate that the requested `output_id` (resolved via `resolve_output_node_id`) is reachable from at least one source. A graph with an Output node disconnected from any oscillator passes validation today and runs silently. Would be more helpful to emit a warning at start.

**Backups**:
- shadow_graph::compile output_buffer_offset overflow audit (256 nodes × MAX_OUTPUTS could overflow u32 only at extreme combinations).
- Frontend (tauri-ui/src/store.ts) error-display surface for the structured set_param errors.
- MIDI queue plumbed but never drained (large feature gap).
- audio_engine.cpp: status_register graph_version/sample_count atomics — verify ordering.
