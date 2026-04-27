# Next loop seed

**Loop 3 candidates (priority order):**

1. **Duplicate-edge rejection in `ShadowGraph::add_edge`.** Currently two
   identical edges from (from_node, from_port) → (to_node, to_port) are
   accepted; C++ then double-mixes the same source into the same input
   slot, doubling level. Bootstrap issue #3.
2. **`ui_main.rs::start_engine` output_node_id is `nodes.len()+1`.** Off-by-
   one / wrong field — should be the id of the user-designated output node
   (the egui UI doesn't currently expose one). Either pick the first node
   whose engine_type == Output, or surface a proper "designate output"
   action. Surfaced by loop 2.
3. **`tauri-ui/src-tauri/src/main.rs::parse_engine_type`** silently maps
   unknown strings to Gain. Bootstrap issue #4.
4. C++ multi-output bug (latent): all `outputs[i]` for one node alias
   `scratch_buffers[slot]`. Latent — no current node exercises it.
5. Broader FFI ABI test coverage (NodeDesc / NodeConnection / CompiledGraph
   alignment + field offsets).

Pick (1) — same priority bucket as loops 1/2 (silent wrong output),
trivial scope, well-isolated, easy to test.
