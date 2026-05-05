# Loop log

Append-only record of every improvement loop. Newest entries at the bottom.

---

## Loop 1 — 2026-05-05

**OBSERVE**
- Repo builds clean. `cargo fmt --check` clean. `cargo clippy -p joduga --all-targets -D warnings` clean. `cargo test -p joduga` → 11 passed, 0 failed.
- Full-workspace clippy needs GTK/WebKit/glib system libs (Tauri); not blocking.

**ORIENT**
- Two concrete, low-risk targets identified in `rust/src/shadow_graph.rs`:
  1. `topological_sort` rebuilds an O(E) edge scan inside the dequeue loop → asymptotic O(V·E). Reusing the adjacency list already needed for cycle detection makes it O(V+E).
  2. Several `add_edge` error branches (unknown source node, unknown target node, output-port OOB, input-port OOB) and `remove_edge`'s "no such edge" branch are not exercised by tests.

**DECIDE**
- Make the topo-sort optimization (no behaviour change, just complexity) and add the missing error-path tests in the same loop, since they touch the same module and reinforce each other.

**ACT**
- Refactored `topological_sort` to take a borrowed adjacency list, with a public wrapper that builds it once. `validate()` and `topological_sort()` now share construction in `compile()`.
- Added 5 new tests: `add_edge_unknown_source`, `add_edge_unknown_target`, `add_edge_output_idx_out_of_bounds`, `add_edge_input_idx_out_of_bounds`, `remove_nonexistent_edge`.

**VERIFY**
- `cargo fmt --all -- --check` ✅
- `cargo clippy -p joduga --all-targets -- -D warnings` ✅
- `cargo test -p joduga --release` → 16 passed, 0 failed ✅ (+5 new tests).

**SEED FOR NEXT LOOP** → see `.agent/next.md`.

---

## Loop 2 — 2026-05-05

**OBSERVE**
- Read `.agent/loop_log.md` and `.agent/next.md`. Loop 1 left a clean repo (`fmt`/`clippy`/`test` all green, 16 tests).
- Top-ranked remaining targets: FFI safety audit (large), lockfree queue stress test, MAX_NODES/MAX_EDGES tests, topo-sort determinism question.

**ORIENT**
- The determinism question is answerable now: roots and per-node neighbour expansion in Kahn's BFS are seeded from `HashMap` iteration, which Rust randomises on every run. That makes `topological_sort` non-deterministic across processes for any graph with sibling roots or fan-in to multiple inputs. FFI consumers (the C++ engine reads `exec_order` directly) benefit from a stable order: same patch → same output every launch.
- MAX_NODES / MAX_EDGES limits are pure tests, no logic change.
- Lock-free queue stress test is doable with `std::thread` only — no new dep.
- FFI safety audit deferred (needs deeper review of unsafe boundaries; out of scope for this loop's "small + verifiable" target).

**DECIDE**
- Bundle three small, related improvements: deterministic topo-sort (sort adjacency lists + initial roots), MAX_*  limit tests, and SPSC cross-thread stress test.

**ACT**
- `rust/src/shadow_graph.rs`: `build_adjacency` now sorts each neighbour list by ID; `topological_sort_with_adj` collects roots into a `Vec`, sorts them, then drives the `VecDeque`. Output is now deterministic.
- Added 3 new tests in shadow_graph: `max_nodes_limit_enforced`, `max_edges_limit_enforced`, `topological_sort_is_deterministic_with_siblings` (latter builds the same graph 32× and asserts identical orderings).
- `rust/src/lockfree_queue.rs`: added `spsc_stress_across_threads` — producer thread sends 200 000 sequential `u32`s; consumer thread asserts they arrive once each, in order. Verifies the documented Acquire/Release contract under real interleaving.

**VERIFY**
- `cargo fmt --all -- --check` ✅
- `cargo clippy -p joduga --all-targets -- -D warnings` ✅
- `cargo test -p joduga --release` → 20 passed, 0 failed ✅ (+4 new tests).

**SEED FOR NEXT LOOP** → see `.agent/next.md` (rewritten).

---

## Loop 3 — 2026-05-05

**OBSERVE**
- Read `.agent/loop_log.md` and `.agent/next.md`. After loops 1+2: 20 tests, all green.
- Top of next.md ranking: FFI safety audit. The lower-effort, immediately-verifiable slice is a layout-assertion test; full extern-fn audit deferred (no behavioural bugs visible from a read of `ffi.rs` and `audio_engine_wrapper.rs`).
- Also flagged: `MAX_NODES`/`MAX_EDGES` were `pub` in `shadow_graph` but not re-exported from the crate root — minor ergonomic gap.

**ORIENT**
- Compared `cpp/include/audio_engine.h` field-for-field against `rust/src/ffi.rs` and `rust/src/lockfree_queue.rs`. Layouts agree, but nothing in CI catches future drift. A `size_of`/`align_of`/offset assertion test pins the contract for free.
- Re-export `MAX_NODES`/`MAX_EDGES` at `joduga::*` is a one-liner.

**DECIDE**
- Add `ffi_layout_matches_cpp` test asserting size and align of every cross-FFI struct + per-field offsets for `NodeDesc` and `NodeConnection`. Re-export the limit constants from `lib.rs`.

**ACT**
- `rust/src/ffi.rs`: added `ffi_layout_matches_cpp` test (sizes 16/16/12/16/16/16, NodeDesc + NodeConnection offsets 0/4/8/12, CompiledGraph alignment matches pointer width).
- `rust/src/lib.rs`: `pub use shadow_graph::{Edge, Node, ShadowGraph, MAX_EDGES, MAX_NODES};`.

**VERIFY**
- `cargo fmt --all -- --check` ✅
- `cargo clippy -p joduga --all-targets -- -D warnings` ✅
- `cargo test -p joduga --release` → 21 passed.

---

## Loop 4 — 2026-05-05

**OBSERVE**
- Loop 3 left the repo green at 21 tests. Next-up in `next.md`: property-based test for `topological_sort`.

**ORIENT**
- The deterministic topo-sort from loop 2 is currently covered by one shaped fixture. A property test exercising 64 random DAGs (≤32 nodes) with an in-tree LCG (no `rand`/`proptest` dep) gives much broader coverage of the invariants: permutation-of-all-nodes, every edge points forward, two consecutive calls produce the same order.

**DECIDE**
- Add a single test, `topological_sort_property_random_dags`, that generates DAGs by emitting only forward edges (lower→higher node ID). Loop 64 random seeds; assert (1) permutation, (2) `pos[from] < pos[to]` for every edge, (3) determinism.

**ACT**
- Added `topological_sort_property_random_dags` to `rust/src/shadow_graph.rs`. Tracks used `(to_node, to_input_idx)` to avoid duplicate-port FFI errors; bounds output_idx and inputs to 32 (each node has 32 ports).

**VERIFY**
- `cargo fmt --all -- --check` ✅
- `cargo clippy -p joduga --all-targets -- -D warnings` ✅
- `cargo test -p joduga --release` → 22 passed, 0 failed.

**SEED FOR NEXT LOOP** → see `.agent/next.md` (rewritten).

---

## Loop 5 — 2026-05-05

**OBSERVE**
- 22 tests, all green from loops 3+4. Re-checked `next.md`: the highest-leverage remaining item is `compile()` integration coverage — the entire C++ engine consumes the output of `compile()` over FFI, but no Rust test verified the output shape end-to-end.
- Also noticed `remove_node` of a missing node was untested (paired error path with `remove_edge`'s nonexistent case from loop 1).

**ORIENT**
- A focused integration test of `compile()` should verify (a) `node_descs` are emitted in `exec_order`, (b) `node_type` round-trips through the desc, (c) every shadow edge appears as a `NodeConnection`, (d) cycles produce an error from `compile()` (not just from `topological_sort` directly).

**DECIDE**
- Add three tests to `shadow_graph::tests`: `remove_nonexistent_node`, `compile_emits_topologically_ordered_descs`, `compile_rejects_cycle`. No production code change.

**ACT**
- Added the three tests. The compile happy-path test asserts `descs[i].node_id == exec_order[i]` for all i and round-trips all 3 edges into the connection set.

**VERIFY**
- `cargo fmt --all -- --check` ✅
- `cargo clippy -p joduga --all-targets -- -D warnings` ✅
- `cargo test -p joduga --release` → 25 passed, 0 failed (+3).

**SEED FOR NEXT LOOP** → see `.agent/next.md` (rewritten).
