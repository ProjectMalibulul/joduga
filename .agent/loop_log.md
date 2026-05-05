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
