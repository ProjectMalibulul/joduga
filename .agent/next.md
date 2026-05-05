# Seed for next loop

This file is overwritten each loop. Read it at the start of the next OBSERVE step.

## Suggested next targets (ranked by value/risk)

1. **Full FFI extern-fn audit** of `rust/src/audio_engine_wrapper.rs` and `rust/src/ffi.rs`:
   - Verify every `extern "C"` call site documents preconditions (non-null, alignment, lifetime).
   - Confirm pointers handed to C outlive the C-side use (Arc / `Box::into_raw` matched by a `from_raw`).
   - Confirm `audio_engine_destroy` is unconditionally called via `Drop` for `AudioEngineWrapper`, even on init partial-failure paths.
   - Layout asserts are now in place (loop 3); next is the lifetime + null-check audit.

2. **C++ DSP denormal/NaN safety** in `cpp/src/nodes/filter.cpp` (biquad/comb): with `-ffast-math` enabled, denormals fed back through state can stay denormal forever and stall on Intel. Add explicit FTZ/DAZ enable at audio thread init in `linux_rt.cpp`/`macos_rt.cpp`/`windows_rt.cpp`.

3. **Workflow polish**:
   - Pin `actions/checkout`, `actions/cache`, `actions/setup-node`, `actions/upload-artifact` to commit SHAs (security best practice).
   - Add a `cargo doc --no-deps --workspace` step gated on warnings.

4. **Tauri frontend (`tauri-ui/src/store.ts`)**: confirm UI gracefully surfaces `invoke` rejections (engine not running, validation failure). Add a TypeScript ESLint config if missing.

5. **`remove_node` edge sweep**: assert the count of remaining edges after removing a hub node, and that the in-degree map is consistent on a subsequent `topological_sort`.

6. **Doc tests**: add `///` examples to public APIs (`ShadowGraph::add_node`, `add_edge`, `compile`) so `cargo test --doc` provides additional smoke coverage.

## Resolved this loop

- ~~`compile()` integration coverage~~ → `compile_emits_topologically_ordered_descs` + `compile_rejects_cycle` (loop 5).
- ~~`remove_node` error path~~ → `remove_nonexistent_node` (loop 5).
- ~~FFI struct layout drift detection~~ → `ffi_layout_matches_cpp` (loop 3).
- ~~Re-export `MAX_NODES`/`MAX_EDGES` at crate root~~ → loop 3.
- ~~Property-based topo test~~ → 64-seed random DAG test (loop 4).

## Hands-off areas

- `rustfmt.toml`, `clippy.toml`: project style — do not change.
- `Cargo.lock`: do not regenerate without reason.
- `.github/agents/*`: off-limits.
- License (`COPYING`/`LICENSE`): repo recently switched MIT → GPLv3; do not touch.
