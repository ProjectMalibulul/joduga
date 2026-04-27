# Loop 8 candidate: tauri-ui mirror of the same Output-resolution bug

The tauri-ui's `src-tauri/src/main.rs` build_engine command has its own
shadow graph construction and may have the same kind of latent bug as
the egui-ui did pre-loop-7. Audit `start_engine_cmd` / `compile_graph`
in tauri-ui/src-tauri/src for:
- Whether output_node_id is derived from a real Output node.
- Whether mode_hash dispatch is hardcoded (lift to param_hash).
- Whether parse_engine_type's Result is fully handled now (loop 4).

Backup candidate: Rust-side smoke test booting a 1-block graph through
AudioEngineWrapper, asserting cpu_load_permil advances and the output
ring fills with non-zero samples.
