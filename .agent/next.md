# Seed for next loop

This file is overwritten each loop. Read it at the start of the next OBSERVE step.

## Suggested next targets (ranked by value/risk)

1. **FFI safety audit** of `rust/src/audio_engine_wrapper.rs` and `rust/src/ffi.rs`:
   - Verify every `extern "C"` function has documented preconditions (non-null, alignment, lifetime).
   - Confirm pointers handed to C outlive the C-side use (Arc / `Box::into_raw` matched by a `from_raw`).
   - Confirm every cross-FFI struct has `#[repr(C)]` and matches the C++ header field-for-field.
   - Add a `#[test]` that compares `std::mem::size_of` and `align_of` against the C side via `bindgen` outputs or hand-written asserts.

2. **C++ DSP denormal/NaN safety** in `cpp/src/nodes/filter.cpp` (biquad/comb): with `-ffast-math` enabled, a single denormal feeding back through the biquad state can stay denormal forever and stall on Intel. Add explicit FTZ/DAZ enable in `linux_rt.cpp`/`macos_rt.cpp`/`windows_rt.cpp` thread-init, OR add a tiny DC bias / flush-to-zero on filter state.

3. **Workflow polish**:
   - Pin `actions/checkout`, `actions/cache`, `actions/setup-node`, `actions/upload-artifact` to commit SHAs (security best practice).
   - Add a `cargo doc --no-deps --workspace` step gated on warnings.

4. **Tauri frontend (`tauri-ui/src/store.ts`)**: confirm UI gracefully surfaces `invoke` rejections (engine not running, validation failure). Add a TypeScript ESLint config if missing.

5. **Property-based test** for `topological_sort` using a tiny in-tree generator (no extra dep): random DAGs with ≤32 nodes, assert the produced order respects every edge (`pos[from] < pos[to]`) and is deterministic across two consecutive calls.

6. **Re-export `MAX_NODES`/`MAX_EDGES` from `lib.rs`** so external callers can size buffers without depending on the private path. Currently they are `pub` on `shadow_graph` but the module visibility chain isn't checked.

## Resolved this loop

- ~~Open question on topo-sort determinism~~ → resolved by sorting adjacency lists and initial roots; covered by `topological_sort_is_deterministic_with_siblings`.
- ~~Lock-free queue concurrency test~~ → added `spsc_stress_across_threads`.
- ~~`MAX_NODES` / `MAX_EDGES` limit tests~~ → added.

## Hands-off areas

- `rustfmt.toml`, `clippy.toml`: project style — do not change.
- `Cargo.lock`: do not regenerate without reason.
- `.github/agents/*`: off-limits.
- License (`COPYING`/`LICENSE`): repo recently switched MIT → GPLv3; do not touch.
