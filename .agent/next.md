# Next loop seed

**Loop 5 candidates:**

1. **C++ multi-output bug.** In `cpp/src/audio_engine.cpp` lines 183-185,
   all `outputs[i]` for one node alias the same `scratch_buffers[slot]`.
   Latent today (every implemented node has num_outputs <= 1) but will
   silently corrupt audio the moment a multi-output node lands.
2. **ui_main.rs `output_node_id = nodes.len() + 1`** (egui-ui only).
   UX-coupled — needs a "designate output" gesture or auto-pick.
3. **Broader FFI ABI tests** (NodeDesc / NodeConnection / CompiledGraph
   alignment + field offsets — generalize the loop-1 fix).
4. **Stronger param-hash hygiene** — Rust hardcodes hex constants; mirror
   `cpp/include/audio_node.h::ParamHash` as a const Rust module so future
   drift is caught at compile time.
5. Param queue truncation guard is dead code (logged, low priority).

Pick (1) — even though latent today, it's a correctness landmine that
will silently corrupt audio the day someone adds a stereo splitter or a
filter with separate L/R outputs. Fixing now while the call site is
small is far cheaper than after multiple node kinds rely on
single-output behaviour.

Investigation TODO before acting: verify the bug is real by reading
`init_node_buffers` / `setup_io_buffers` (whatever wires `outputs[i]`),
confirm `scratch_buffers` layout, and decide whether the fix is
"allocate scratch per (slot, output_idx)" or "introduce a separate
output-buffer pool indexed by global output id".
