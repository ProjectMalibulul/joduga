# Loop 7 candidate: ui_main.rs:1135 off-by-one (output_node_id = nodes.len() + 1)

Fix the latent bug discovered in loop 2: the egui-ui auto-assigns
`output_node_id = nodes.len() + 1` when adding nodes, but node IDs are
zero-indexed so `nodes.len()` (not +1) is the correct next ID. With the
loop-2 validate() check now in place this drift would surface as a
"output node not found" error at compile time instead of silent silence.

Approach:
1. Trace the assignment in ui_main.rs around line 1135.
2. Determine whether the +1 was compensating for a 1-indexed scheme
   somewhere — if so, fix the root, not the symptom.
3. Add a unit test exercising the node-add path (extract logic to a pure
   helper if needed; ui_main.rs is feature-gated egui-ui so the test must
   sit under the same cfg).

Backup candidate: C++ engine smoke test from Rust — boot a 1-block
graph through AudioEngineWrapper, assert ring fills, assert
cpu_load_permil advances. Requires no audio device because the wrapper
runs the engine's audio thread directly.
