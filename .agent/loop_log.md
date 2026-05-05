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
