# Next loop seed (loop 35)

The realtime DSP path and the host startup path are now hardened. Drilling into the live param-update path next.

**Loop 35 candidate**: cpp/src/audio_engine.cpp param-drain at lines 153-167. Currently the loop:
1. Loads tail with Acquire.
2. Loads head with Acquire (should be Relaxed — it's the consumer-owned index).
3. Computes `avail`.
4. Drains into `pending_params`.
5. Stores tail with default ordering.

The Acquire on the consumer's own head is a wasted barrier — Acquire only matters for the producer's tail (it synchronizes with the producer's Release). The store of the new tail at the end should be Release (publishes "we're done with these slots") — currently ordering not specified in this snippet, may default to seq_cst which is over-strong.

Audit and align with the SPSC pattern documented in DESIGN.md / lockfree_queue.rs.

**Backup**:
- midi_input.rs reconnect/disconnect race with the queue producer side.
- shadow_graph.rs edge cap of 1024 — is overflow detected at add_edge or only at compile?
- Frontend (tauri-ui/src/store.ts) error display: the new structured set_param error from loop 34 needs a UI surface.
