# Next loop seed (loop 41)

Validation surface across the host/middleware boundary is now comprehensive:
  - node existence (loop 39)
  - param queue backpressure (loop 34)
  - Output node existence + type + incoming edge (loops 2, 40)
  - cpal device default channels (loop 36)
  - MIDI parser data-byte mask (loop 37)

**Loop 41 candidate**: shadow_graph::compile output_buffer_offset overflow audit. Each NodeDesc gets `output_buffer_offset[slot] = running_total`; running_total grows by num_outputs each step. With 256 nodes × MAX_AUDIO_OUTPUTS each, the total fits in u32 trivially, but the C++ side uses `uint32_t buf_idx = output_buffer_offset[from_slot] + from_output` and indexes scratch_buffers — should confirm that scratch_buffers is sized correctly.

**Backups**:
- Frontend (tauri-ui/src/store.ts) error-display surface for the structured set_param errors.
- MIDI queue plumbed but never drained (large feature gap).
- audio_engine.cpp: status_register graph_version atomic ordering.
