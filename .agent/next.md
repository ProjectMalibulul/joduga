# Next loop seed

**Loop 2 target:** `ShadowGraph::validate` does not require `output_node_id`
to exist as a node. Engine then sets `output_feeder_slot = -1` and emits
silence with no error reported. Add validation:

- In `rust/src/shadow_graph.rs::validate`, after existing checks, return
  `Err` if `self.output_node_id` is set but not present in `self.nodes`.
- Decide whether `output_node_id == 0` / unset is also rejected (probably
  yes — a graph with no output is useless).
- Add tests: missing output rejected; valid output passes.
- Consider whether `compile` should also re-check, or rely on `validate`
  being called first by `AudioEngineWrapper::start`.

Open from loop 1: broader FFI ABI test coverage (NodeDesc / NodeConnection /
CompiledGraph alignment + field-offset asserts) — logged, not blocking.
