# Joduga — Copilot Instructions

Joduga is a real-time node-based audio synthesizer with a three-layer architecture: a **Tauri + React Flow** frontend, a **Rust** middleware (graph validation, FFI, cpal output, MIDI), and a **C++20** DSP engine that owns the real-time audio thread.

## Architecture (the parts that span files)

Data flows top-down; the audio thread never crosses back into Rust:

1. **UI (`tauri-ui/src/`)** — React Flow canvas. `catalog.ts` defines 67 UI node templates that map to **only 4 engine node types** (Oscillator, Filter, Gain, Output). Extra "node types" are parameter presets, not new C++ classes. State lives in `store.ts` (Zustand) and is sent to Rust via Tauri IPC.
2. **Rust middleware (`rust/src/`)**
   - `shadow_graph.rs` — validates topology, detects cycles, runs Kahn's topological sort. Hard caps: **256 nodes, 1024 edges**. C++ trusts the order it receives.
   - `ffi.rs` — `#[repr(C)]` structs (`NodeDesc`, `NodeConnection`, `ParamUpdateCmd`) and `extern "C"` bindings. **Every FFI struct must be `repr(C)`; every FFI fn `extern "C"`.**
   - `lockfree_queue.rs` — SPSC ring buffer. Producer uses `Relaxed` on its own index, `Acquire` on the remote index, and `Release` to publish. C++ mirrors the pattern. Power-of-2 capacity, mask-based modulo. **No other synchronization primitives are allowed between Rust and the audio thread.**
   - `audio_engine_wrapper.rs` — owns the queues and the cpal output stream that drains the C++ ring buffer.
   - `midi_input.rs` — `midir` listener that pushes events into a separate SPSC queue.
   - `main.rs` is a headless CLI test harness; `ui_main.rs` is a standalone egui debug UI (gated by the `egui-ui` feature). The shipping app entry point is `tauri-ui/src-tauri/src/main.rs`.
3. **C++ engine (`cpp/`)** — `audio_engine.cpp` runs the audio thread (`platform/{linux_rt,macos_rt,windows_rt}.cpp` set SCHED_FIFO / Mach time-constraint / THREAD_PRIORITY_TIME_CRITICAL respectively). Block size is **256 samples**. New node types are added by:
   1. extending the `NodeType` enum in **both** `rust/src/ffi.rs` and `cpp/include/audio_engine.h` (values must match),
   2. adding the class under `cpp/include/nodes/` + `cpp/src/nodes/`,
   3. wiring it into the `case` in `create_node()` in `audio_engine.cpp`,
   4. adding the new `.cpp` to `CMakeLists.txt`'s `joduga_audio` static library list.

Parameters are addressed by **FNV-1a hash** of their name (see `ParamHash` namespace in `cpp/include/audio_node.h`). UI/Rust must hash with the same algorithm before sending `ParamUpdateCmd`.

## Real-time invariants (do not violate)

The audio callback must be **wait-free, allocation-free, syscall-free**:
- No `new`/`malloc`/`std::vector::push_back`, no mutexes, no virtual dispatch added to the hot path (existing virtuals are pre-resolved at graph compile).
- Queue overflow drops new updates rather than blocking — preserve this behavior.
- Coefficients (filters, etc.) are recomputed **once per block**, not per-sample; parameters are smoothed across the block.
- All node buffers and state are pre-allocated in the constructor / on graph compile.

## Build, test, lint

The Rust workspace (`Cargo.toml` → members `rust`, `tauri-ui/src-tauri`) drives the C++ build via the `cmake` build-dependency in `rust/build.rs`. You don't invoke CMake directly for normal work.

```bash
# Full workspace build (also compiles libjoduga_audio.a via CMake)
cargo build --workspace --release

# Headless audio test (Osc -> Filter -> Gain -> Output)
cargo run --release --bin joduga

# Tauri dev (Vite on :1420 + Rust + C++)
cd tauri-ui && cargo tauri dev

# Tauri production bundle
cd tauri-ui && cargo tauri build

# Tests — full suite or a single test
cargo test --workspace --release
cargo test --workspace --release -- shadow_graph::tests::detects_cycle   # single test by path
cargo test -p joduga lockfree_queue                                       # single crate + filter

# Lint gate (matches CI exactly — must pass with zero warnings)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings

# Lock-free queue throughput benchmark
cargo run --example bench_queue -p joduga --release
```

Linux build deps (CI uses these; replicate locally):
`build-essential cmake g++ pkg-config libasound2-dev libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libsecret-1-dev`.

CI (`.github/workflows/ci.yml`) gates on `fmt --check` + `clippy -D warnings` first, then a Linux/macOS/Windows matrix that runs `cargo build`, `cargo test`, `cargo tauri build`, and on Linux **verifies `libjoduga_audio` is statically linked** (`ldd` must not show it). Don't switch the C++ archive to a shared library.

## Conventions

- **Rust edition 2021**, formatted with `rustfmt.toml` (max width 100, `use_small_heuristics = "Max"`). Clippy `type-complexity-threshold = 300` (see `clippy.toml`).
- **C++20**, `-ffast-math -ftree-vectorize` (Linux/macOS), `/fp:fast /arch:AVX2` on MSVC x86_64. `POSITION_INDEPENDENT_CODE ON` is required because the static lib is linked into a PIE binary.
- The `joduga` crate has feature `egui-ui` (default on) which pulls in `eframe`/`egui` for the standalone debug UI. The `joduga-tauri` crate depends on `joduga` with `default-features = false` — keep non-Tauri-safe deps gated behind `egui-ui`.
- `NodeType` enum values are an ABI contract: same integer values in `rust/src/ffi.rs` and `cpp/include/audio_engine.h`. Don't reorder existing variants.
- Rust owns all FFI buffers; C++ holds non-owning raw pointers. On drop, stop the engine first, then free buffers.
- Branches `main`, `test/cd`, `debug/core` trigger CI on push; PRs target `main`.

## Reference docs in repo

`DESIGN.md` (full architecture + memory-ordering rationale), `QUICKSTART.md` (worked example of adding a Delay node end-to-end), `TROUBLESHOOTING.md` (SCHED_FIFO permissions, linker errors), `DOCS_INDEX.md` (map of the rest).
