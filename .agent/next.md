# Loop 9 candidate: Rust-side smoke test for AudioEngineWrapper

Boot a 1-block graph through AudioEngineWrapper, assert the output
ring fills with non-zero samples (oscillator at known freq) and that
cpu_load_permil advances. Tests the actual FFI and C++ engine path
without requiring real audio hardware (cpal). Requires:
- A test-only constructor or a way to drive the engine without cpal,
  or pulling samples directly out of `output_ring()`.
- Knowledge of how the engine populates the ring on its own thread.

Backup candidate: Extract the duplicated `resolve_output_node_id`
helpers (egui + tauri) into a shared `joduga::output_resolver` module
parameterised by a small trait so both call sites can reuse it.
