# Next loop seed (loop 39)

Host-side concurrency now scales with concurrent UI calls. Outstanding:

**Loop 39 candidate**: `set_param` Tauri command does not validate that `node_id` exists in the running engine. A stale id from a freshly-rebuilt UI graph would be silently enqueued for a node that doesn't exist — the C++ param drain skips unknown ids, but the user sees no diagnostic.

**Backups**:
- shadow_graph::compile output_buffer_offset overflow audit.
- Frontend (tauri-ui/src/store.ts) error-display surface.
- MIDI queue plumbed but never drained (large feature gap).
- Tauri command `set_param` busy-loops the front-end with `?` errors when queue is briefly full — should we provide a non-fatal "param dropped due to backpressure" return value?
