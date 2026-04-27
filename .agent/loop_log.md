# Loop log

(Append-only. Newest at the bottom.)

## Loop 1 — FFI ABI alignment for ParamUpdateCmd / MIDIEventCmd

**Observe:** Baseline 11/11 tests green. C++ declares both command structs
`alignas(16)`; Rust had only `#[repr(C)]` (4-byte align). `Vec<T>` allocates
at `align_of::<T>()`, so the queue backing buffers were 4-aligned while the
C++ ABI contract said 16-aligned. Silent ABI violation.

**Decide:** Pick alignment fix over (b) output-node existence validation and
(c) duplicate-edge rejection. Highest leverage: fixes a contract violation
in the FFI layer that all subsequent loops will rely on.

**Devil's advocate:**
- *Correctness*: `align(16)` cannot reduce alignment, only raise it; size is
  unchanged because both structs are exactly 4 × u32 with no trailing padding;
  C++ already aligned to 16 so the two sides now agree. Vec stride matches
  size so element addressing is unaffected; only the base pointer becomes
  16-aligned, which is what we want.
- *Scope*: Root cause is missing centralized FFI ABI test coverage. Mitigated
  here by the new alignment+size assertions next to the structs; broader ABI
  surface (NodeDesc/NodeConnection/CompiledGraph) is logged for a later loop.
- *Priority*: Issue #2 (silent silence on missing output node) is more
  user-visible, but ABI corruption produces non-reproducible failures that
  cost more debugging. Trivial to fix, so do it first; #2 is loop 2.

**Act:** `rust/src/lockfree_queue.rs`: changed both `#[repr(C)]` to
`#[repr(C, align(16))]`. Added `cmd_structs_match_cpp_alignas_contract`
test asserting align=16, size=16 for both structs and that
`LockFreeRingBuffer::as_ptr()` returns a 16-aligned pointer.

**Verify:** `cargo test -p joduga --lib` → 12 passed (11 prior + 1 new).
`cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings`
clean.

## Loop 2 — ShadowGraph::validate rejects missing output node

**Observe:** Loop 1 committed cleanly, 12/12 tests + lint green. Bootstrap
issue #2: `validate` did not require `output_node_id` to exist; engine then
sets `output_feeder_slot=-1` and emits silence with no error reported.

**Decide:** Three candidates: (a) output-node existence check in validate,
(b) duplicate-edge rejection in add_edge, (c) parse_engine_type strictness.
Pick (a): top of priority order (logic bug producing silently wrong output
on user-reachable path), single-file change, blocks compile() from ever
crossing the FFI with an invalid output target.

**Devil's advocate:**
- *Correctness*: Could break existing tests that call validate? Audited:
  `detect_cycle` uses `new(0)` and adds node 0; `linear_chain` etc. all use
  output ids that match an added node. None broken. `compile()` calls
  `validate()` at line 168 so the new check protects every consumer.
- *Scope*: Root cause is "no validation that the output target is real".
  C++ side could also error/log (defense-in-depth) but doing it in Rust
  prevents the bad config crossing FFI at all — preferable. Logged C++
  side as future loop candidate.
- *Priority*: Discovered while wiring this in: `rust/src/ui_main.rs:1134`
  uses `output_node_id = nodes.len() + 1`, which never matches any node id
  (off-by-one + uses count where it should use the actual output node id).
  Latent bug that the egui UI was silently hiding; now surfaces as a
  Validation error string. Logged for a future loop. The Tauri UI
  (`tauri-ui/src-tauri/src/main.rs:125`) reads `output_id` from the
  payload, so it is unaffected.

**Act:** `rust/src/shadow_graph.rs::validate`: prepend a
`contains_key(&self.output_node_id)` check returning a descriptive error.
Added three tests: missing output rejected, empty graph rejected, compile
also rejects.

**Verify:** `cargo test -p joduga --lib` → 15 passed (12 prior + 3 new).
`cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings`
clean.

## Loop 3 — ShadowGraph::add_edge rejects exact duplicate edges

**Observe:** Loops 1-2 committed cleanly, 15/15 tests + lint green. Bootstrap
issue #3: add_edge accepted identical (from_node, from_port, to_node,
to_input) tuples; C++ engine sums all incoming connections, so a duplicate
edge silently doubles the source's level on that input.

**Decide:** Three candidates: (a) duplicate-edge rejection, (b) ui_main
output_node_id off-by-one fix, (c) parse_engine_type strictness. Pick (a):
priority-tier match with prior loops (silent-wrong-output bug), trivial
scope, single file, well-isolated test surface. (b) requires UX decision
about how the egui UI designates an output and is logged. (c) is the
clear next step after this.

**Devil's advocate:**
- *Correctness*: Could legit graphs hit this? Same source → same
  destination port is by definition duplicate; the legit case is same
  source → *different* ports (mono-to-stereo fanout) which differs in
  `to_input_idx`. Added `parallel_edges_to_distinct_ports_are_allowed`
  test pinning that. Could it false-positive on (from, to) only? No —
  the predicate compares all four fields.
- *Scope*: Root cause is that C++ sums duplicates without knowing they
  are duplicates. Fixing C++ to dedupe at engine-init would be more
  defense-in-depth, but the canonical answer is: don't let invalid
  edges cross the FFI in the first place. Same precedent as loop 2.
- *Priority*: ui_main off-by-one is real but UX-blocked; this is a
  clean correctness fix. Proceeding.

**Act:** Added duplicate-tuple check at the end of `add_edge`'s validation
in `rust/src/shadow_graph.rs`. Two new tests: `duplicate_edge_rejected`
and `parallel_edges_to_distinct_ports_are_allowed`.

**Verify:** `cargo test -p joduga --lib` → 17 passed (15 prior + 2 new).
`cargo fmt --check` clean. `cargo clippy --workspace --all-targets -- -D warnings`
clean.

## Loop 4 — parse_engine_type returns Result instead of silently coercing

**Observe:** Loops 1-3 committed cleanly. Workspace tests 17/17 + lint green.
Bootstrap issue #4: `parse_engine_type` in the Tauri backend silently
mapped any unknown engine_type string to `NodeType::Gain`, turning a
frontend serialization bug into a silent wrong-engine-type bug at runtime.

**Decide:** Three candidates: (a) make parse_engine_type fallible and
propagate, (b) ui_main output_node_id off-by-one (UX-coupled), (c) C++
multi-output bug (latent). Pick (a): direct correctness fix, single
function + one caller, testable as a pure helper without the GUI runtime.

**Devil's advocate:**
- *Correctness*: Could legit input now fail? Only if the React side
  serializes a name that isn't in the catalog — by definition that is the
  bug we want to surface, not hide. The IPC contract from `tauri-ui/src`
  uses these exact 7 strings; any drift is a frontend bug. The `?` at
  the call site converts the error into the existing `Result<(), String>`
  return of the `start_engine` Tauri command, which the frontend already
  handles. Verified caller list with grep — only one site to update.
- *Scope*: Root cause is "stringly-typed FFI between TS and Rust". A
  proper fix would mirror `NodeType` as a serde enum; logged for a future
  loop. Today's change is the minimum that flips silent-success into
  loud-error and is therefore strictly safer than the status quo.
- *Priority*: ui_main off-by-one (loop 5 candidate) is real but UX-
  coupled. C++ multi-output is latent. This is the right next pick.

**Act:** `tauri-ui/src-tauri/src/main.rs`: changed `parse_engine_type` to
return `Result<NodeType, String>`. Updated the single caller in
`start_engine` to use `?`. Added a `tests` module with two tests:
known-strings round-trip and unknown/empty/case-mismatched are rejected.

**Verify:** `cargo test --workspace` → 17 lib tests + 2 new joduga-tauri
tests, all green. `cargo fmt --check` clean. `cargo clippy --workspace
--all-targets -- -D warnings` clean.

## Loop 5 — C++ engine: per-output scratch buffers (remove multi-output aliasing)

**Observe:** Loops 1-4 committed cleanly, 19 tests + lint green. Bootstrap
issue #5: `audio_thread_main` set every `outputs[i]` for one node to
`scratch_buffers[slot].data()`, so a node with num_outputs > 1 would
silently overwrite output 0 when writing output 1. Companion bug:
`SlotConn` did not carry `from_output_idx` at all, so even with separate
output buffers there was no plumbing to route them.

**Decide:** Three candidates: (a) full per-output scratch refactor + add
from_output to SlotConn, (b) assert num_outputs <= 1 and document, (c)
broader FFI ABI tests. (b) loses information and just defers the problem.
(c) is testable but lower-priority. Pick (a): minimal, behaviorally
identical for single-output (every existing C++ node hardcodes
num_outputs=1), and unblocks future stereo / multi-output nodes.

**Devil's advocate:**
- Correctness: regression risk on single-output? Every implemented C++
  node sets num_outputs=1; per-output offsets degenerate to one buffer
  per node — identical data flow, different index. New edge sanity
  check only drops edges that violate `from_output_idx < num_outputs`
  which Rust-side add_edge already enforces against the descriptor.
- Scope: real root cause is scratch buffer keyed only by slot, not by
  port. Fixed at source.
- Priority: no automated C++ engine test means structural-identity
  reasoning is the best we have. Logged adding a Rust-side smoke test
  as the next loop.

**Act:** `cpp/src/audio_engine.cpp`:
- Added `from_output` to `SlotConn`; populate from `c.from_output_idx`
  with a bounds check against the resolved C++ node's num_outputs.
- Replaced per-slot scratch_buffers with per-output buffers indexed via
  new `output_buffer_offset[slot]` table; total size = sum of
  node->num_outputs.
- Renamed `output_feeder_slot` -> `output_feeder_buffer`, now an index
  into the flat scratch_buffers array.
- Updated audio thread to compute input/output pointers via
  `output_buffer_offset[from_slot] + from_output` and `+ i`.

**Verify:** C++ rebuilt cleanly via `cargo test --workspace` (build.rs
invokes the cmake crate). 17 lib tests + 2 tauri tests pass. Lint clean.

## Loop 6 — Centralize Rust mirror of cpp ParamHash table

**Observe:** main.rs uses bare `0x811C_9DC5` hex literals for osc/filter
freq updates; ui_main.rs declares its own private `H_FREQ`/`H_RES`
constants (also bare hex). The C++ side has a rich ParamHash namespace
(WAVEFORM_TYPE, FILTER_MODE, THRESHOLD, RATIO, etc.) with no Rust
counterpart. Future Rust callers (or the existing main.rs) can drift
silently from the C++ table — a typo on either side routes a parameter
update into the wrong dispatch arm or gets dropped.

**Decide:** Three candidates: (a) full Rust mirror module + tests +
migrate existing call sites, (b) extract H_FREQ/H_RES constants to lib
without the rest, (c) write an FFI integration test. (b) under-fixes —
half the table stays missing. (c) requires running an audio device.
Pick (a): pure Rust, deterministic, surfaces drift as unit-test failures,
removes magic numbers from main.rs.

**Devil's advocate:**
- Correctness: numerical values copied from cpp/include/audio_node.h ─
  test re-asserts canonical values so any future C++-side rename without
  a Rust update fails CI loudly.
- Scope: real risk is C++/Rust drift on add. Mitigation: a comment block
  pointing both files at each other, plus the canonical-value test.
- Priority: an FFI smoke test (next loop) is the only thing higher,
  and it requires audio hardware to be useful.

**Act:** New `rust/src/param_hash.rs` mirroring all 26 C++ ParamHash
constants with disjoint-hash invariant test. Re-exported via
`joduga::param_hash`. Replaced bare hex in main.rs with `param_hash::
OSC_FREQUENCY` / `param_hash::FILTER_CUTOFF`. Replaced ui_main.rs
local constants with re-exports of `joduga::param_hash::FREQ`/`RES`.

**Verify:** 19 lib tests (was 17) — 2 new param_hash tests pass.
cargo build --bins green. cargo fmt --check green. cargo clippy
--all-targets -D warnings green. No call site behavior change ─
constants are byte-identical to the literals they replaced.

## Loop 7 — Fix ui_main.rs Output node resolution (and lift mode hashes)

**Observe:** Three tightly-coupled bugs in the egui-ui (default-feature)
`start_engine` path:
1. `ShadowGraph::new(max_nodes as u32)` passed `nodes.len() + 1` as the
   `output_node_id` arg — semantically wrong; with the loop-2 validate
   tightening this now hard-fails on any normal user graph (Output node
   id rarely equals `nodes.len()+1`).
2. AudioEngineWrapper got a *separately* computed `output_id` that
   silently fell back to `0` when no Output node existed — engine then
   reads from arbitrary node 0 instead of erroring.
3. The mode_hash dispatch table (Oscillator → 0xAD, Filter → 0xBD, etc.)
   was hand-typed hex instead of the loop-6 param_hash constants.

**Decide:** Three candidates: (a) fix all three together — one helper
resolves Output id, used both for ShadowGraph::new and the wrapper;
swap mode_hash to param_hash; add tests; (b) fix only #1; (c) fix only
the silent-fallback. (b)/(c) leave the same class of bug in another
spot. Pick (a).

**Devil's advocate:**
- Correctness: helper rejects "no Output" and "multiple Output" cases
  with explicit messages — both surface as visible status text instead
  of a silent wrong-engine-state. ShadowGraph::new and the wrapper now
  receive the same id so they cannot disagree.
- Scope: real root cause is "id was guessed from container length";
  fixed at source by deriving from the Output node itself. Mode hashes
  were a separate latent drift risk also lifted to the canonical table.
- Priority: this is the user-visible default-feature start path —
  highest priority remaining after loops 1-6.

**Act:** ui_main.rs:
- New `resolve_output_node_id(nodes, catalog) -> Result<u32, String>`
  helper (rejects missing/duplicate Output and bad template indices).
- start_engine resolves output_id once and reuses it.
- mode_hash literals replaced with `joduga::param_hash::*` references.
- New `tests` mod with 4 unit tests covering happy path, missing
  Output, duplicate Outputs, and dangling template_idx.

**Verify:** cargo test --bins → 4/4 ui_main tests pass; lib still 19/19.
fmt clean. clippy --all-targets -D warnings clean.

## Loop 8 — tauri-ui: same Output-resolution + mode_hash drift as loop 7

**Observe:** Audit of tauri-ui/src-tauri/src/main.rs::start_engine_cmd
revealed the same two-bug pattern fixed in loop 7's egui-ui:
1. `output_id = nodes.iter().find(|n| n.engine_type == "Output").map(|n| n.id).unwrap_or_else(|| nodes.last().map(|n| n.id).unwrap_or(0))`
   silently substitutes "the last node" or "node id 0" when the user
   forgets an Output. After loop-2's validate hardening this is either
   a confusing graph error or — worse — an audio output reading from
   an arbitrary non-Output node.
2. mode_hash dispatch hardcoded as `0xAD/0xBD/0xCF/0xCD/0xCE` instead of
   the loop-6 param_hash constants.

**Decide:** Mirror loop 7 exactly — extract a `resolve_output_node_id`
helper, fail fast on missing/duplicate, swap mode_hash to param_hash,
add tests. Other candidates (extract one shared helper into the joduga
crate; expose JS-facing error to the UI) would require introducing a
shared types crate or touching the frontend, both higher-cost.

**Devil's advocate:**
- Correctness: helper is a near-clone of the egui-ui's; both fail with
  the same human-readable strings so users see consistent errors.
- Scope: the duplication is a smell — both helpers should eventually
  live in one place. Lifting them needs a shared types crate (the input
  shapes differ: GraphNode vs EngineNodeInfo). Logged as a future loop.
- Priority: this is the user-visible Tauri start path; equal priority
  to loop 7. Done now.

**Act:** tauri-ui/src-tauri/src/main.rs:
- New `resolve_output_node_id(&[EngineNodeInfo]) -> Result<u32, String>`.
- start_engine_cmd uses it with `?`.
- mode_hash literals lifted to `joduga::param_hash::*`.
- 3 new unit tests; existing parse_engine_type tests untouched.

**Verify:** cargo test --workspace → 28 tests (19 lib + 4 ui_main + 5
tauri). cargo fmt --check + cargo clippy --workspace --all-targets
-D warnings both green.

## Loop 9 — End-to-end C++ engine smoke test from Rust

**Observe:** Until now every test was static — struct alignment, graph
validation, slug parsing. The C++ DSP path itself (oscillator, filter,
gain, output, scratch buffers) had zero automated coverage. Loop 5
refactored the per-output scratch buffer layout and could only be
verified by structural reasoning. Future C++ engine changes have the
same blind spot.

**Decide:** Three candidates: (a) integration test booting a 1-node
oscillator → output graph through AudioEngineWrapper without cpal,
asserting the ring fills with non-zero samples; (b) extract
`resolve_output_node_id` duplication into a shared module; (c) add an
ABI-layout test for NodeDesc / NodeConnection. (b) is cleanup. (c) is
useful but lower-impact than (a) — without (a) we cannot detect
engine-side regressions at all. Pick (a).

**Devil's advocate:**
- Correctness: AudioEngineWrapper exposes `output_ring()` (Arc<OutputRingBuffer>)
  and `read()` is safe — same path cpal uses, just driven by a test
  thread instead. `audio_engine_start` spawns the audio thread on its
  own (verified in audio_engine.cpp), so we don't need cpal.
- Scope: smoke test asserts only "ring fills + nonzero samples"; we
  intentionally don't pin frequency or amplitude exactly because the
  engine's internal oscillator amplitude is implementation-defined.
  cpu_load_permil is also not asserted — too noisy on fast CIs.
- Priority: this is the highest-impact test we can add with current
  infrastructure. It also retroactively validates loops 5-8.

**Act:** New `rust/tests/engine_smoke.rs` integration test:
- Builds Osc(0) → Output(1) graph, validates, compiles.
- Boots AudioEngineWrapper without cpal, sets osc freq via
  param_hash::OSC_FREQUENCY, starts engine, sleeps 120 ms.
- Reads from output_ring(), asserts at least one sample arrived and
  max abs amplitude > 1e-4.
- Re-tunes osc to verify the param queue stays drainable.
- Drops engine (audio_engine_destroy via Drop).

**Verify:** Test passes (took ~120 ms wallclock). Workspace test
totals: 19 lib + 1 smoke + 4 ui_main + 5 tauri = 29. fmt + clippy
--all-targets -D warnings clean. C++ engine actually produces
non-zero samples through the new per-output buffer layout from loop 5
— retroactive structural confirmation.

## Loop 10 — ABI-layout pinning tests for FFI structs

**Observe:** loop 1 added an alignment test for ParamUpdateCmd /
MIDIEventCmd, but NodeDesc, NodeConnection, AudioEngineConfig, and
CompiledGraph — also shared with C++ via FFI — had no layout tests.
A field reorder on either side would silently mis-route data: e.g.
swap `num_inputs` and `num_outputs` in NodeDesc and the engine would
allocate the wrong number of input ports for every node.

**Decide:** Three candidates: (a) size+align+offset_of tests for all
four FFI structs; (b) a single round-trip test that writes a NodeDesc
in Rust and reads it back via a tiny C++ helper; (c) lift the
ParamHash duplication. (b) is overkill — std::mem::offset_of! gives
the same coverage for a tenth of the cost. Pick (a). Skip (c) again —
it's plumbing, not a defect.

**Devil's advocate:**
- Correctness: relies on Rust's repr(C) actually agreeing with the
  C++ side's typedef struct. Both are documented as #[repr(C)] /
  C-typedef structs; the test pins the bytes either way.
- Scope: CompiledGraph layout depends on pointer width. Test gated on
  target_pointer_width=64 — the only platform the engine builds for.
  32-bit case left unchecked rather than wrong.
- Priority: lower than loop 9's smoke test but higher than the cleanup
  candidates. Done now while the FFI is fresh.

**Act:** Extended `rust/src/ffi.rs::tests` with 4 new layout tests
using std::mem::offset_of! (stable since 1.77). Each test cites the
C++ header line that must be kept in sync.

**Verify:** 23 lib tests (was 19) + 1 smoke + 4 ui_main + 5 tauri
= 33 total. fmt + clippy --workspace --all-targets -D warnings clean.

## Loop 11 — offset_of pinning for cmd/status structs

**Observe:** loop 1 pinned alignment (16) and total size of
ParamUpdateCmd / MIDIEventCmd, but field offsets were unchecked.
StatusRegister had no layout test at all, despite the C++ audio
thread doing AtomicU32::from_ptr on cpu_load_permil at offset 8.

**Decide:** Add offset_of! tests for ParamUpdateCmd, MIDIEventCmd,
and StatusRegister, completing the FFI layout coverage that loop 10
started.

**Devil's advocate:**
- Correctness: Rust #[repr(C)] with all-u32 fields is forced to
  natural-order 0/4/8/12 — but that's exactly what the assertion
  pins, so a #[repr(Rust)] slip-up surfaces immediately.
- Scope: MIDIEventCmd has no formal C++ counterpart yet; pinning
  is anticipatory but cheap, and the alignment test from loop 1
  already commits to 16-byte layout.
- Priority: lower than a behavioural test, but completes a coherent
  ABI-coverage pass and the cost is three short tests.

**Act:** Added param_update_cmd_field_offsets_match_cpp,
midi_event_cmd_field_offsets, and status_register_field_offsets_match_cpp
in rust/src/lockfree_queue.rs::tests.

**Verify:** 26 lib tests (was 23). fmt + clippy clean.

## Loop 12 — C++ static_assert mirror of FFI layout

**Observe:** loops 10-11 pinned the FFI struct layout from the Rust
side. But a C++-side reorder only surfaces when someone happens to
run `cargo test` — the C++ build itself stays green. Asymmetric
coverage.

**Decide:** Add static_assert(offsetof / sizeof / alignof) lines in
cpp/include/audio_engine.h mirroring every Rust layout assertion
from loops 1, 10, 11. Closes the symmetry: a drift on either side
fails its own language's build.

**Devil's advocate:**
- Correctness: `offsetof` on standard-layout structs is well-defined
  in C++; all five FFI structs are POD/aggregates. CompiledGraph
  layout is pointer-width dependent, so guard with
  UINTPTR_MAX == 0xFFFFFFFFFFFFFFFFu like the Rust cfg.
- Scope: this is symmetry, not a new behavioural test. Justified
  because the FFI is the binary-compatibility surface — drift here
  is silent corruption, exactly the priority-1 class.
- Priority: completes the loop-10/11 pass with no remaining gap.
  Cheap, mechanical, high signal.

**Act:** Appended a "ABI layout guards" block after the extern "C"
in cpp/include/audio_engine.h, covering NodeDesc, NodeConnection,
AudioEngineConfig, ParamUpdateCmd, StatusRegister, and 64-bit-gated
CompiledGraph.

**Verify:** cmake --build cmake-build -j builds clean (every TU
that includes audio_engine.h evaluates the asserts). 26 lib tests
green, clippy clean.

## Loop 13 — JodugaApp default-graph silent fallback

**Observe:** Loops 7-8 fixed silent unwrap_or fallbacks in start_engine
but `JodugaApp::new()` still had three of the same anti-pattern:
  flt_idx  = position("Low-Pass Filter").unwrap_or(14)
  gain_idx = position("Gain").unwrap_or(32)
  out_idx  = position("Speaker Output").unwrap_or(cat.len() - 1)
If the catalog is reorganized, the demo graph silently boots with
wrong node types — flt_idx=14 might be a Reverb, gain_idx=32 might be
out of range, and the user hears garbled audio with no diagnostic.

**Decide:** Replace each `unwrap_or(magic)` with a `.unwrap_or_else(panic!)`
naming the missing template — same rationale as loops 7-8 (fail fast on
catalog drift). Add a unit test pinning the four template names so a
catalog refactor surfaces as a test failure instead of a runtime panic.

**Devil's advocate:**
- Correctness: panicking at app startup is louder than the silent
  garbled-audio path. Acceptable: catalog drift is a developer bug, and
  shipping such a build through CI now requires the new test to fail.
- Scope: the deeper issue is that the demo graph identifies templates
  by name string. Fixing that (e.g. enum-keyed catalog) is a larger
  refactor; the panic-with-name behaviour solves the immediate
  silent-corruption problem. Logged for a future loop.
- Priority: priority-1 silent-corruption class — exactly the same
  bug-shape as loops 7-8.

**Act:** rust/src/ui_main.rs::JodugaApp::new — replaced three
unwrap_or fallbacks with a `find` closure that panics naming the
missing template. Added default_graph_templates_exist_in_catalog
test in the same module.

**Verify:** 26 lib + 5 ui_main + 1 smoke = 32 joduga tests. fmt +
clippy clean.

## Loop 14 — ShadowGraph dfs_cycle phantom-node fallback

**Observe:** shadow_graph.rs:168 had
`match color.get(&next).copied().unwrap_or(0)` inside the cycle-detection
DFS. add_edge validates both endpoints, but `nodes`/`edges` are pub —
external code can splice an Edge directly into `edges` referencing a
node that isn't in the node map. The unwrap_or(0) silently treats such
phantom endpoints as WHITE and recurses into them, growing the color
map past the node count and producing nonsensical cycle reports
(or none) for malformed graphs that the engine then refuses.

**Decide:** Two-part fix. Add edge endpoint validation in validate()
(every `from`/`to` must be a known node), then upgrade the dfs's
unwrap_or(0) to .expect() naming the now-true invariant. Two new tests
exercise the validation directly via the pub `edges` field.

**Devil's advocate:**
- Correctness: validate() now does an O(E) endpoint scan. Cheap; the
  cycle DFS already does O(V+E). No regression for valid graphs.
- Scope: the deeper issue is the pub fields. Making them private would
  be a larger refactor; the current fix at least guarantees compile()
  rejects malformed graphs at the validate() boundary.
- Priority: priority-1 — current behaviour is silent acceptance of a
  graph the engine will later mishandle.

**Act:** rust/src/shadow_graph.rs::validate — added edge endpoint
loop. dfs_cycle — replaced unwrap_or(0) with .expect() naming the
invariant. Tests: validate_rejects_edge_with_unknown_source_node,
validate_rejects_edge_with_unknown_target_node.

**Verify:** 28 lib tests (was 26). fmt + clippy clean.
