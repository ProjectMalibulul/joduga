# Bootstrap — initial repo analysis

Captured by the autonomous improvement agent on its first OBSERVE pass.

## Repo at a glance

- **Project:** Joduga — real-time node-based audio synthesizer.
- **Languages:** Rust (middleware + FFI + UI shell), C++20 (audio engine), TypeScript/React (Tauri frontend).
- **Workspace layout:**
  - `rust/` — main Rust crate (`joduga`), exposes `cdylib` + `rlib`, two bins (`joduga` CLI, `joduga-ui` egui).
  - `cpp/` — static C++ audio engine (`libjoduga_audio.a`) linked into the Rust binary via the `cmake` build crate.
  - `tauri-ui/` — Tauri 2 + React Flow + Zustand frontend; its own `src-tauri` Rust crate is part of the Cargo workspace.
- **Top-level build:** `Cargo.toml` workspace + `CMakeLists.txt` for the C++ engine. Profile: `release` uses `lto = true`, `codegen-units = 1`.

## Build & verification commands (verified working in CI sandbox)

System deps (Ubuntu 24.04): `build-essential cmake g++ pkg-config libasound2-dev libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libsecret-1-dev`.

- `cargo fmt --all -- --check` — clean ✅
- `cargo clippy -p joduga --all-targets -- -D warnings` — clean ✅ (only needs `libasound2-dev`)
- `cargo clippy --workspace --all-targets -- -D warnings` — needs full GTK/WebKit deps (Tauri)
- `cargo test -p joduga --release` — 11 tests pass ✅
- Tauri: `cd tauri-ui && cargo tauri dev|build`

## CI

`.github/workflows/`:
- `ci.yml` — lint+fmt gate, then 3-OS matrix build (Linux/macOS/Windows) with full Tauri build and static-link verification on Linux.
- `nightly.yml`, `release.yml` — nightly + tag-driven release.

## Source-level notes

- `rust/src/lockfree_queue.rs` — SPSC ring buffer with documented Acquire/Release ordering. Tests cover enqueue/dequeue, full/empty, wraparound, len.
- `rust/src/shadow_graph.rs` — graph validation (DFS cycle detection) + Kahn's topological sort. Compiles to `NodeDesc`/`NodeConnection` for FFI.
  - **Hotspot:** `topological_sort` re-scans `self.edges` inside the BFS loop → O(V·E). The adjacency list is rebuilt independently in `validate()`. Both can share one adjacency build, dropping topo to O(V+E).
  - **Coverage gap:** error branches in `add_edge` (unknown source, unknown target, out-of-bounds port indices) and `remove_edge`'s "no such edge" path are not exercised by tests.
- `rust/src/ffi.rs`, `audio_engine_wrapper.rs`, `midi_input.rs`, `main.rs`, `ui_main.rs` — not yet deeply audited.
- C++ engine and Tauri UI not yet deeply audited.

## Conventions observed

- Rust 2021 edition, `rustfmt.toml` and `clippy.toml` present (use them; do not change style).
- 4-space indent in Rust; small functions are condensed onto fewer lines (e.g., `Self { nodes: ..., edges: ..., output_node_id }`).
- All Rust public APIs use `Result<_, String>` for shadow-graph errors.
- Tests live in `#[cfg(test)] mod tests` at the bottom of the same file.
- No `unsafe` in safe modules except where memory-ordering contract is documented (lockfree_queue).

## Pending audits (for future loops)

- `audio_engine_wrapper.rs` — FFI safety review.
- `ffi.rs` — `extern "C"` ABI surface.
- C++ DSP nodes (`cpp/src/nodes/*.cpp`) — denormal handling, NaN safety.
- Tauri `src-tauri/src/main.rs` — IPC commands.
- Frontend `tauri-ui/src/store.ts` — error handling on `invoke` failures.
- Workflow files — pinned action versions, caching strategy.
