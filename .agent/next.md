# Seed for next loop

This file is overwritten each loop. Read it at the start of the next OBSERVE step.

## Suggested next targets (ranked by value/risk)

1. **Audit `rust/src/audio_engine_wrapper.rs` and `ffi.rs`** for FFI safety:
   - Confirm every `extern "C"` function has documented preconditions.
   - Check that all C-side pointers passed to Rust outlive the Rust-side accessors (Arc/lifetimes).
   - Verify `repr(C)` on every cross-FFI struct.

2. **Lock-free queue concurrency test**: add a thread-spawned producer/consumer test that drains N=1M items across threads with `loom` or a simple stress test (no extra dep — just `std::thread`).

3. **C++ DSP denormal/NaN safety audit**: verify `cpp/src/nodes/filter.cpp` (biquad/comb) flushes denormals (FTZ/DAZ on x86, `-ffast-math` already enabled but check inputs).

4. **Workflow polish**:
   - Pin third-party actions to commit SHAs (security — `dtolnay/rust-toolchain@stable` is fine but `actions/checkout@v4`, `actions/cache@v4`, `actions/setup-node@v4`, `actions/upload-artifact@v4` could be SHA-pinned).
   - Add a separate `cargo test --doc` step (currently 0 doc tests; encourage doc examples).

5. **Tauri frontend (`tauri-ui/src/store.ts`)**: confirm graceful UI behaviour when `invoke` rejects (engine not running, validation failure). Add a TypeScript-level lint config if missing.

6. **`MAX_NODES` / `MAX_EDGES` limits**: untested. Add tests that the 257th node and 1025th edge are rejected with the expected message.

## Open questions

- Should `topological_sort` be made stable (deterministic order across HashMap iteration)? Currently relies on iteration order of `HashMap`, which is randomized. The `linear_chain` and `non_contiguous_ids` tests happen to pass because each level has only one node. With siblings, order is non-deterministic. Consider switching to `BTreeMap` or sorting roots/queue, OR documenting that order is unstable across equivalent topologies.

## Hands-off areas

- `rustfmt.toml`, `clippy.toml`: project style — do not change.
- `Cargo.lock`: do not regenerate without reason.
- `.github/agents/*`: off-limits.
