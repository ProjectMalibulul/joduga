# Bootstrap — Repo state at start of agent run

Branch: `test/cd` (also `origin/test/cd`). Working tree clean except untracked
`.github/copilot-instructions.md` from the previous task and this `.agent/`
directory. Last commit: `9a0fe6c Fix formatting using rustfmt`.

## Stack

- **Rust workspace** (edition 2021): `rust/` (`joduga` crate, lib + 2 bins) and
  `tauri-ui/src-tauri/` (`joduga-tauri`). Resolver 2. LTO + codegen-units=1 in
  release. `egui-ui` feature gates `eframe`/`egui`/`egui_plot`.
- **C++20** static library `libjoduga_audio.a`, built by `cmake` crate from
  `rust/build.rs`. Sources in `cpp/`. Static-linked into Rust binaries (CI
  enforces this with `ldd`).
- **Tauri 2** + React 18 + React Flow + Zustand frontend in `tauri-ui/`.

## Tests baseline

`cargo test --workspace --release` → **11 passed, 0 failed** (all in
`joduga` lib: `ffi::tests::node_type_repr`, six `shadow_graph::tests::*`,
four `lockfree_queue::tests::*`). No tests at all in `joduga-tauri`, no
C++ tests, no integration tests.

CI gate (`.github/workflows/ci.yml`): `cargo fmt --check`, `cargo clippy
--workspace --all-targets -- -D warnings`, then matrix build + test +
`cargo tauri build` + `ldd` static-link check on Linux.

## Mental model

Three layers, top-down only:

1. **Frontend** (`tauri-ui/src/*`) — React Flow canvas, 67 UI node types in
   `catalog.ts` map onto **7 engine NodeType variants** (Oscillator, Filter,
   Gain, Output, Delay, Effects, Reverb). Zustand `store.ts`. IPC commands:
   `start_engine`, `stop_engine`, `set_param`, `get_engine_cpu_load_permil`.
2. **Rust middleware** (`rust/src/*`)
   - `shadow_graph.rs` — `ShadowGraph` with HashMap of nodes, Vec of edges,
     Kahn's topo sort, DFS cycle detection. Caps: 256 nodes, 1024 edges.
   - `ffi.rs` — `#[repr(C)]` `NodeType`, `NodeDesc`, `NodeConnection`,
     `CompiledGraph`, `AudioEngineConfig`, opaque `AudioEngine`. `extern "C"`
     prototypes for `audio_engine_init/start/stop/destroy/...`.
   - `lockfree_queue.rs` — generic `LockFreeRingBuffer<T: Copy>` with Arc
     `AtomicUsize` head/tail. Producer Relaxed-Acquire-Release. Power-of-2
     capacity, masked indices. `ParamUpdateCmd`/`MIDIEventCmd`/`StatusRegister`
     all `#[repr(C)]`.
   - `audio_engine_wrapper.rs` — owns boxed param/midi queues, `StatusRegister`,
     and `Arc<OutputRingBuffer>`. Builds `CompiledGraph` from boxed slices,
     calls `audio_engine_init`, then drops the slices (C++ copies the graph).
   - `midi_input.rs` — `midir` listener pushing into the MIDI queue.
3. **C++ engine** (`cpp/src/audio_engine.cpp`)
   - `AudioEngineImpl` holds nodes, `node_id_to_slot`, `execution_order`,
     `slot_connections`, scratch buffers, queue pointers, output ring.
   - `audio_thread_main` runs at SCHED_FIFO. Drains param queue, processes
     graph in exec_order, copies output node's scratch buffer to ring,
     deadline-paces with `sleep_precise_ns`. Updates `cpu_load_permil` via
     `std::atomic_ref<uint32_t>` on the shared `StatusRegister`.

## Notable behavior already in place

- Param hash convention: FNV-like u32 constants in `cpp/include/audio_node.h`
  mirrored as bare `0x811C_9DC5` etc. in Rust callers. The Rust side has **no
  central hash table** — UIs and tests literally hardcode the hex.
- `output_feeder_slot = -1` if `output_node_id` not in node map → silent run.
- C++ stores output ring head/tail as `std::atomic<size_t>*` pointing into
  `OutputRingBuffer`'s `AtomicUsize` fields; OutputRingBuffer is heap-stable
  inside an `Arc`.

## Issues observed (not yet acted on)

These are the candidate problems for the loop, ordered by my current best
guess at impact:

1. **`ParamUpdateCmd` / `MIDIEventCmd` ABI alignment mismatch.** C++ declares
   both with `alignas(16)`; Rust declares `#[repr(C)]` only — 4-byte alignment.
   `LockFreeRingBuffer<T>::buffer` is a `Vec<T>`, allocated at `align_of::<T>()`.
   Result: queue slots are 4-aligned in memory while the C++ ABI contract says
   they are 16-aligned. C++ `std::atomic` is not used on the structs, but any
   future SIMD/`atomic_ref<__m128>` would UB; even today this is a silent ABI
   contract violation that only happens to work on x86_64 because unaligned
   16-byte loads are tolerated. **Fix is trivial and high-leverage.**
2. **`ShadowGraph::compile` / `validate` does not require `output_node_id` to
   exist as a node.** The engine then has `output_feeder_slot = -1` and emits
   silence with no error reported anywhere. UI users will see the "Play" button
   work but hear nothing.
3. **`ShadowGraph::add_edge` does not reject duplicate edges.** Two identical
   edges from the same (from_node, from_port) to the same (to_node, to_port)
   are accepted; C++ then double-mixes the same source into the same input
   slot, doubling its level. UI deduping is not guaranteed.
4. **`tauri-ui/src-tauri/src/main.rs::parse_engine_type`** silently maps
   unknown strings to `NodeType::Gain` — a frontend bug becomes a silent
   wrong-node-type bug in the engine.
5. **C++ `audio_engine.cpp` multi-output bug (latent).** All `outputs[i]` for
   one node are written to the same scratch buffer (`scratch_buffers[slot]`).
   Today every implemented node has `num_outputs <= 1`, so this is dead-latent
   but will silently corrupt audio the moment a multi-output node is added.
6. **Param queue drain in `audio_engine.cpp` always advances tail by `avail`,
   even if `pending_params.size() < avail` would have truncated.** Currently
   `pending_params` is sized to queue capacity, so this can never trigger;
   still worth noting.
7. **`shadow_graph::topological_sort` allocates fresh HashMaps and re-scans
   `self.edges` for every queue pop** — O(V+E²) instead of O(V+E). Performance
   is irrelevant at 256 nodes / 1024 edges, so this is style-only.

Loop priority: tackle 1 → 2 → 3 → 4 → 5 in order. 6/7 stay logged.
