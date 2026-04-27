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

## Loop 15 — End-to-end param-queue test for non-Oscillator nodes

**Observe:** The loop 9 smoke test only sets OSC_FREQUENCY. If
ParamUpdateCmd dispatch were broken for any node type other than
Oscillator (different switch in set_param, wrong hash routing, etc.)
the test would still pass. The actual end-to-end param plumbing for
GainNode / FilterNode / etc. had no behavioural coverage.

**Decide:** Add a second smoke test that targets the Output GainNode
specifically. Set GAIN_LEVEL=0 mid-flight, wait for the smoother to
settle, and assert the next window's amplitude has dropped >95% from
the pre-cut window. Proves: param queue routes by node_id correctly,
GainNode set_param dispatches GAIN_LEVEL, and smoothing converges in
the documented window.

**Devil's advocate:**
- Correctness: smoothing constant 0.99 → 99.9% settled in ~14 ms; the
  test waits 60 ms and discards that tail before measuring window 2.
  Threshold loud*0.05 leaves >10× margin against measurement jitter.
- Scope: still only covers GainNode (which the Output node aliases
  to). FilterNode/EffectsNode/Reverb/Delay still untested. But this
  exercises a fundamentally different param-hash than OSC_FREQUENCY
  and a different dispatch path, so it does close one full failure
  mode that the previous smoke test couldn't see.
- Priority: priority-3 (test gap on existing functionality). The
  GainNode dispatch is hot in real use (every Output node). Keeping
  it untested was a real gap.

**Act:** rust/tests/engine_smoke.rs — added
output_node_gain_param_silences_stream. Reuses the same Osc→Output
graph but mutates Output's GAIN_LEVEL and asserts amplitude collapse.

**Verify:** 2/2 smoke tests pass (both ~110 ms wallclock). 28 lib +
5 ui_main + 2 smoke = 35 joduga tests. fmt + clippy clean.

## Loop 16 — End-to-end FilterNode dispatch + multi-hop routing test

**Observe:** Loops 9 and 15 covered Oscillator and Gain set_param
dispatch and a 2-node graph. FilterNode dispatch (FILTER_CUTOFF /
FILTER_MODE / FILTER_RESONANCE / etc.) and any multi-hop routing
through an interior node had zero tests. The multi-hop case directly
exercises the per-output buffer offsets introduced in loop 5.

**Decide:** Add a third smoke test: Osc(0) → Filter(1) → Output(2).
Set OSC_FREQUENCY=8 kHz and FILTER_MODE=LP. With cutoff=20 kHz the
filter is transparent; window 1 should be loud. Drop cutoff to 100 Hz,
wait for the filter's per-block 5% smoother to converge (time constant
~107 ms), and assert window 2 amplitude is < 25% of window 1. Three
distinct dispatch paths now have behavioural coverage (OSC_FREQUENCY,
GAIN_LEVEL, FILTER_CUTOFF) and the interior-node routing path has its
first test.

**Devil's advocate:**
- Correctness: first attempt used 80 ms wait — the test failed
  legitimately (stop=0.61, ~38% attenuation, filter still mid-transit).
  Diagnosed: per-block 5% smoothing → 107 ms time constant. Increased
  to 350 ms tail + 150 ms window (>4× time constant). Test now passes
  comfortably under threshold.
- Scope: doesn't cover Reverb/Delay/Effects, but those have many
  parameters; one canonical filter param test per node type is the
  right cadence for follow-up loops.
- Priority: test gap on existing functionality (priority 3) — and
  doubles as the first behavioural verification of loop 5's per-output
  buffer routing.

**Act:** rust/tests/engine_smoke.rs —
filter_node_cutoff_attenuates_high_frequency_source. 3-node graph,
sets and re-sets FILTER_CUTOFF mid-flight, asserts amplitude collapse.

**Verify:** 3/3 smoke tests pass (~620 ms total wallclock). 28 lib +
5 ui_main + 3 smoke = 36 joduga tests. fmt + clippy clean.

## Loop 17 — ShadowGraph: enforce output_node_id is an Output-type node

**Observe:** validate() checks that the output_node_id key exists in
nodes but never checks that the node's NodeType is Output. A user (or
JSON import / bad refactor) could declare an Oscillator's id as the
output, and the C++ engine would happily treat that oscillator's
output buffer as the speaker sink — bypassing every downstream effect
silently. Same bug class as loop 7-8 (silent wrong-output routing), at
the validation layer instead of the resolver layer.

**Decide:** Add a NodeType::Output assertion in validate() right after
the existence check. Add a test that constructs ShadowGraph::new(0)
with id=0 = Oscillator and confirms validate() returns the descriptive
error.

**Devil's advocate:**
- Correctness: every existing test that uses validate() routes through
  an Output-typed node by id, so no regressions. Confirmed: 36/36
  joduga tests still green (29 lib + 5 ui_main + 3 smoke + 0 doctests).
- Scope: doesn't enforce that the Output node has num_inputs ≥ 1, but
  add_node already constructs Output with num_inputs=1 in every
  callsite seen. Logged for future audit.
- Priority: priority-1 silent-corruption — perfect mirror of the
  validation gap that loops 7-8 fixed at the resolver layer.

**Act:** rust/src/shadow_graph.rs::validate — added type-check for
`self.nodes[&self.output_node_id].node_type`. Added test
validate_rejects_non_output_typed_sink.

**Verify:** 29 lib + 5 ui_main + 3 smoke = 37 joduga tests. fmt +
clippy clean.

## Loop 18 — C++ audio_engine_init: defensive guards on FFI inputs

**Observe:** Rust's ShadowGraph::validate now blocks malformed graphs
(loop 17), but the C++ audio_engine_init() FFI is exposed as plain C
ABI and may be embedded by other hosts. Today it null-checks only
`graph` and `config`. Specifically:
- output_node_id is stored without verifying it resolves to a node;
  the per-block ring-feed lookup silently no-ops, producing permanent
  silence with no diagnostic.
- block_size could be 0 → empty scratch buffers, every node's
  process() iterates 0 frames forever, again silent.
- The lock-free queues use `(cap-1)` as a power-of-two mask; a
  non-PoT capacity would wrap incorrectly and leak param/MIDI cmds.
- `nodes`, `connections`, `execution_order` pointers could be null
  while their counts are non-zero → segfault inside the init loop.

**Decide:** Add 4 init-time guards before any heap allocation, returning
nullptr with a stderr line for each. Add 2 Rust integration tests that
bypass ShadowGraph and drive AudioEngineWrapper::new directly with
malformed inputs.

**Devil's advocate:**
- Correctness: each guard runs once at boot, no audio-path cost. The
  output_node_id check happens AFTER node_id_to_slot is built so it
  catches both unknown ids and not-yet-created entries.
- Scope: not a symptom fix — silent corruption is exactly what these
  guards prevent. Loop 17 caught it on the Rust side; this catches it
  at the same layer Rust calls into.
- Priority: priority-1 silent-corruption / null-deref. Worth the
  surface area added to the FFI contract.

**Act:**
- cpp/src/audio_engine.cpp: 4 guard blocks at top of audio_engine_init,
  plus output_node_id resolution check after node map is built.
- rust/tests/engine_smoke.rs: 2 new tests
  (cpp_init_rejects_unresolved_output_node_id,
   cpp_init_rejects_zero_block_size).

**Verify:** cmake build clean. 29 lib + 5 ui_main + 5 smoke = 39 joduga
tests pass. fmt + clippy clean.

## Loop 19 — MIDI parser: NoteOn vel=0 → NoteOff + queue-full diagnostic + 9 unit tests

**Observe:** rust/src/midi_input.rs::dispatch is the only entry path
for external MIDI events. Audit found:
- **Logic bug**: NoteOn with velocity=0 was emitted as a NoteOn event.
  Per MIDI 1.0 spec and decades of running-status convention, virtually
  every keyboard sends 0x90 N 0x00 instead of 0x80 N V to terminate
  notes. The current code re-triggers the held note instead of
  releasing it. Affects every user with a typical hardware keyboard.
- **Silent drops**: `let _ = queue.enqueue(cmd)` discards events when
  the queue fills, with no log or counter. Burst input on a slow audio
  thread silently loses notes.
- **Zero test coverage**: dispatch() and its parsing are fully untested.

**Decide:** Extract pure `parse(msg) -> Option<MIDIEventCmd>` from
dispatch so the parsing logic is testable without midir. Translate
NoteOn vel=0 → NoteOff in parse. Add stderr warning on queue-full in
dispatch. Add 9 unit tests covering: vel=0 conversion, normal NoteOn,
NoteOff, CC, pitch bend bit-packing, empty/truncated/sysex inputs,
channel-nibble stripping.

**Devil's advocate:**
- Correctness: vel=0 → NoteOff is the unambiguously correct
  interpretation per the MIDI spec; tests demonstrate both directions.
- Scope: this is the cause, not a symptom — the synth nodes downstream
  treat NoteOn / NoteOff distinctly, and would otherwise need to know
  about the running-status convention themselves. Better to normalise
  at the boundary.
- Priority: priority-4 logic bug on a critical path (every keyboard
  note goes through here). Plus priority-3 (test gap) closed.

**Act:** rust/src/midi_input.rs — added parse(), updated dispatch to
log queue-full, added 9-test #[cfg(test)] mod.

**Verify:** 38 lib tests (was 29; +9 midi tests) pass. fmt + clippy
clean.

**Open follow-up:** queue-full log is unrate-limited and could spam
stderr under sustained overload. A status_register.dropped_midi_count
counter would be the right shape but requires touching the FFI ABI;
deferred to a future loop.

## Loop 20 — Smoke test: cpu_load_permil populates under load

**Observe:** StatusRegister.cpu_load_permil is computed every block in
audio_engine.cpp:225-229 and surfaced via cpu_load_permil() on the
wrapper, but no test asserts the engine actually populates it. A
silent breakage of this telemetry would only manifest as a flat UI
load meter — easy to miss.

**Decide:** Add a smoke test with a heavy enough graph (Osc → Filter →
Reverb → Output) that proc_ns lands above the per-mille rounding floor
even on a fast CI runner. Assert load > 0 && < 4000 (the C++-side
clamp). Existing 1-osc test was deliberately weak per its own
docstring; this one is the heavy counterpart.

**Devil's advocate:**
- Correctness: the assertion (0, 4000) is loose enough to cover both
  fast and slow runners. Reverb has internal delay buffers so it's
  guaranteed to do real work.
- Scope: closes a test gap on existing functionality (priority 3).
- Priority: telemetry is what users see when something is off; we
  should know if the field stops updating.

**Act:** rust/tests/engine_smoke.rs — added
`cpu_load_permil_advances_under_load`, exercises 4-node chain through
heaviest available DSP nodes.

**Verify:** 38 lib + 5 ui_main + 6 smoke = 49 joduga tests pass. Ran
in ~0.62 s including the 200 ms wall-time wait. fmt + clippy clean.

## Loop 21 — audio_engine_start: prevent double-start std::terminate crash

**Observe:** While auditing audio_engine_wrapper for safety holes,
inspected audio_engine_start (cpp:424). The implementation:
```
e->is_running.store(true, ...);
e->audio_thread = std::thread(audio_thread_main, e);
```
If called when already running, the move-assignment over a *joinable*
std::thread invokes std::terminate per the C++ standard. That is a
priority-1 latent crash. Currently the Rust UI calls start() once
during init, but any future "restart engine" or "reload graph" feature
would hit this. audio_engine_stop is symmetric — calling stop twice on
a freshly stopped engine attempts join() on a non-joinable thread,
which is a no-op only because of the `joinable()` check.

**Decide:** Replace the unconditional store with compare_exchange_strong
to atomically transition stopped↔running. Start returns -2 if already
running; stop is idempotent and returns 0 if already stopped. Add a
Rust integration test that calls start() twice and asserts the second
errors, and confirms double-stop is safe.

**Devil's advocate:**
- Correctness: CAS gives single-winner semantics. Memory order
  acq_rel for success means the audio thread sees the transition
  before the std::thread launch, and on the stop path the join() is
  ordered after the false→true transition fails for losers.
- Scope: this IS the cause; the previous unwrap-style store was the
  primitive. Stop's symmetric CAS prevents a future second-stopper
  from racing the joiner.
- Priority: priority-1 (process crash via std::terminate). Closing
  this matters even if no current caller triggers it — defense in depth
  at the C ABI boundary, exposed to any host.

**Act:**
- cpp/src/audio_engine.cpp::audio_engine_start: CAS + return -2.
- cpp/src/audio_engine.cpp::audio_engine_stop: CAS + idempotent no-op.
- rust/tests/engine_smoke.rs::double_start_is_safe_and_reports_error.

**Verify:** cmake build clean. 38 lib + 5 ui_main + 7 smoke = 50
joduga tests pass. fmt + clippy clean.

## Loop 22 — Audio thread liveness watchdog via graph_version

**Observe:** status_register.graph_version is incremented by the C++
audio thread every block (cpp:222) but the Rust wrapper exposed no
accessor. If the audio thread hung — deadlock, panic in a node —
nothing in the host could detect it. The output ring would drain to
silence and the UI would keep painting like nothing was wrong.

**Decide:** Add `graph_version()` and `is_audio_thread_alive(timeout)`
to AudioEngineWrapper. The latter samples the counter, sleeps, and
returns whether it advanced — a primitive any host can poll. Add a
smoke test that exercises both directions of the boolean (advancing
while running, frozen after stop).

**Devil's advocate:**
- Correctness: graph_version is fetch_add'd atomically on the C++ side
  via std::atomic_ref; Rust reads it via AtomicU32::from_ptr with
  Acquire ordering. No tear, no UB.
- Scope: a watchdog primitive, not a watchdog policy. The current
  task scope is to make the condition observable; making the host
  *react* (alarm, restart, etc.) is host-level work and out of scope
  for the engine wrapper.
- Priority: priority-2 (missing error handling on a critical path —
  audio thread liveness was unmonitored). The cost of the helper is
  zero on the audio path; only the host pays the sleep cost.

**Act:**
- rust/src/audio_engine_wrapper.rs: graph_version() +
  is_audio_thread_alive() helpers.
- rust/tests/engine_smoke.rs: audio_thread_liveness_via_graph_version
  smoke test.

**Verify:** 38 lib + 5 ui_main + 8 smoke = 51 joduga tests. fmt +
clippy clean.

## Loop 23 — 2025-01-XX

**OBSERVE**: Audited `cpp/include/nodes/oscillator.h`. Test counts at start: 38 lib + 8 smoke = 46. Loop 22 watchdog landed clean; no pending mid-flight work.

**ORIENT**: Found a priority-1 silent-corruption bug in the FM/AM cases. `set_param` clamped `OSC_FREQUENCY` to [0.01, 20000] but `FM_MOD_FREQ`/`AM_MOD_FREQ` were stored verbatim. The per-sample mod_phase advance is `TWO_PI * mod_freq * sample_rate_inv`. With `mod_freq = 1e9` at 48 kHz that's ≈ 1.3e8 rad/sample — far exceeding `TWO_PI` — and the wrap was a single-step `if (mod_phase > TWO_PI) mod_phase -= TWO_PI;` which only normalises increments smaller than TWO_PI. Net effect: `mod_phase` grows unboundedly, `sin(mod_phase)` loses precision, FM/AM output decays into shaped garbage. NaN propagation through downstream filters/reverb is plausible.

Same `if`-wrap exists for the carrier `phase` and `saw_phases`, but carrier is bounded by the existing 20 kHz frequency clamp (per-sample increment ≈ 2.62 rad < TWO_PI), and `saw_phases` is a lower-impact follow-up (deferred to next.md).

**DECIDE**: Two paired fixes in `oscillator.h::set_param`:
1. Clamp `FM_MOD_FREQ`/`AM_MOD_FREQ` to [0, 20000] (mirrors carrier clamp) and `FM_MOD_DEPTH`/`AM_MOD_DEPTH` to [0, 100] (depth in radians; large depths alias but won't blow up the accumulator).
2. Replace the FM/AM `if` wraps with `while` loops as defense-in-depth — even if a future change relaxes the clamp, the accumulator stays bounded.

**DEVIL'S ADVOCATE**:
- *Correctness*: Clamping silently drops user requests for >20 kHz mod-freq. Defensible — anything above Nyquist (24 kHz @ 48 kHz SR) aliases anyway, and the carrier clamp is the same. The `while` is safe because per-sample increment ≤ 2.62 rad after clamping; the loop runs ≤1 iteration in steady state. *No standing.*
- *Scope*: Fix addresses root cause (unbounded accumulator), not a symptom. The carrier-phase `if` wrap is structurally identical but safe under existing clamps; SUPER_SAW's `saw_phases` wrap is also vulnerable when paired with unclamped `detune` — added to next.md as loop-24 candidate. *Partially stands → flagged for follow-up, not bundled.*
- *Priority*: This is silent garbage / potential NaN propagation (priority 1). No untouched higher-priority issue is known. *No standing.*

**ACT**:
- `cpp/include/nodes/oscillator.h`: clamp 4 params (FM_MOD_FREQ, FM_MOD_DEPTH, AM_MOD_FREQ, AM_MOD_DEPTH); `if` → `while` in FM and AM mod_phase wraps.
- `rust/tests/engine_smoke.rs`: new `fm_oscillator_with_extreme_mod_freq_stays_bounded` — drives an FM oscillator with `FM_MOD_FREQ = 1e9` and `FM_MOD_DEPTH = 5.0`, then asserts every sample is finite and bounded by 1.0 + ε.
- Pre-existing fmt drift + new clippy `explicit_auto_deref` warning on the loop-22 `cpu_load_permil` rewrite (`(*self.status_register).cpu_load_permil` → `self.status_register.cpu_load_permil` since the field is a `Box`, not raw pointer). Fixed in-place since they were direct dependencies of CI staying green.

Result: 38 lib + 9 smoke + 5 ui_main = **52 tests pass**. fmt + clippy clean.


## Loop 24 — SUPER_SAW detune accumulator blowup

**OBSERVE**: 38 lib + 9 smoke at start. Loop 23 fixed FM/AM mod_phase wrap; next.md flagged the same pattern in SUPER_SAW.

**ORIENT**: `SUPER_SAW` per-voice update: `saw_phases[j] += TWO_PI * frequency * (1 + detune_amt) * dt` with `detune_amt = (j - v/2) * detune * 0.01`. `detune` was unclamped (line 109: `detune = value;`). With voices=7, j=6, frequency=20000, detune=1e6 → factor=3001, increment ≈ 7854 rad/sample. Single-step `if (saw_phases[j] > TWO_PI)` wrap can't normalise → unbounded phase → `(p/TWO_PI)` overflows representable floats → garbage.

**DECIDE**: Mirror loop 23's fix on the detune path: clamp DETUNE to [0, 1] (matches the 0-1 UI control documented in DESIGN.md), and convert the SUPER_SAW phase wrap from `if` to `while`.

**DEVIL'S ADVOCATE**:
- *Correctness*: Clamping ≥1.0 changes nothing for sane inputs (typical detune is 0.1-0.3). After clamp, max increment per voice ≈ TWO_PI * 20000 * 1.03 / 48000 ≈ 2.69 rad < TWO_PI; while-loop runs ≤1 iteration. *No standing.*
- *Scope*: Carrier `phase` wrap remains `if`, but it's safe under the existing 20 kHz frequency clamp and is documented in next.md as defense-in-depth follow-up. *Acknowledged, not bundled.*
- *Priority*: priority-1 silent corruption identical to loop 23. *No standing.*

**ACT**:
- `cpp/include/nodes/oscillator.h::set_param`: clamp `DETUNE` to [0, 1].
- `cpp/include/nodes/oscillator.h::process` SUPER_SAW: `if` → `while` on saw_phases wrap.
- `rust/tests/engine_smoke.rs`: `super_saw_with_extreme_detune_stays_bounded` (waveform=11, freq=20kHz, detune=1e6 — pre-fix would overflow per-voice phase; post-fix clamp + while keeps output finite & bounded).

Result: 38 lib + **10 smoke** + 5 ui_main = **53 tests pass**. fmt + clippy clean.


## Loop 25 — Filter biquad state-explosion + FILTER_MODE NaN UB

**OBSERVE**: 38 lib + 10 smoke at start. Audited `cpp/include/nodes/filter.h`.

**ORIENT**: Two priority-1 silent-corruption issues:

1. **State-explosion bug** in the DF-II-T biquad processing loop. The soft-clip clamped `y` to ±4.0 *after* using the unclipped `y` to update `z1`/`z2`. Under high resonance an unstable pole pair could drive state unboundedly while output looked clamped. Once z1/z2 reached non-finite values, every subsequent sample was poisoned (output stuck at NaN or alternating ±4 with NaN-coloured state). The comment "Soft-clip to prevent explosions at high resonance" was aspirational — the code didn't actually do that.

2. **`mode = static_cast<int>(value)`** in `FILTER_MODE`'s set_param path with no NaN/Inf guard. Per the C++ standard, casting a non-finite or out-of-range float to int is *undefined behavior*. Rust callers passing `f32::NAN` or 1e9 through the param queue trip UB on the audio thread.

**DECIDE**:
1. Reorder the biquad inner loop: clip `y` *first*, then update z1/z2 with the clipped value. Add a NaN-recovery guard that scrubs z1/z2/y to zero if a non-finite slips in (so a single poisoned sample cannot lock the filter).
2. In `FILTER_MODE`'s set_param branch: reject non-finite values and clamp the integer cast to `[0, VOWEL]`.

**DEVIL'S ADVOCATE**:
- *Correctness*: Clip-before-state changes filter behavior only when |y|>4 (already pathological — well outside any musical signal). NaN scrub is a one-sample glitch, far better than permanent silence/garbage. Mode clamp falls through to `default: case LP:` which the switch already handles. *No standing.*
- *Scope*: Real root cause for unstable poles is sometimes the resonance ceiling itself (Q=30 is borderline for some modes); a deeper fix would be per-mode coefficient stability checks. State clamp is symptom mitigation matching the existing intent — defensible as the smallest correct fix. *Acknowledged, follow-up flagged in next.md.*
- *Priority*: priority 1 (silent NaN/Inf in audio path → speaker damage candidate downstream). *No standing.*

**ACT**:
- `cpp/include/nodes/filter.h` set_param: NaN/range guard on FILTER_MODE.
- `cpp/include/nodes/filter.h` process: reorder clip → state update; add `std::isfinite(z1) && std::isfinite(z2)` recovery scrub.
- `rust/tests/engine_smoke.rs`: `filter_high_resonance_state_remains_bounded` (Osc 440 Hz → Filter LP cutoff=440 Q=30 → Output; verify finite + ≤4.0 + ε); `filter_mode_rejects_nan_and_out_of_range` (set FILTER_MODE = NaN, then 1e9; verify no crash, finite output).

Result: 38 lib + **12 smoke** + 5 ui_main = **55 tests pass**. fmt + clippy clean.


## Loop 26 — Reverb audio-thread allocation + NaN poisoning

**OBSERVE**: 38 lib + 12 smoke at start. Audited `cpp/include/nodes/reverb.h`.

**ORIENT**: Two priority-1 issues, one a real-time-discipline violation:

1. **Audio-thread allocation**: `set_param(DELAY_TIME, …)` called `set_delay_lengths`, which called `lines[i].assign(n, 0.0f)` for each of the 4 FDN delay vectors. `std::vector::assign` reallocates when n exceeds capacity — and the constructor only sized buffers to the *initial* room_size (≈0.07 s of delay), so any later DELAY_TIME automation crossing into a larger room reallocated under the global allocator lock. This is the textbook RT-discipline bug the project's design.md flags repeatedly: priority inversion → audio-thread blocked → xrun.

2. **NaN poisoning of FDN state**: The 4-line FDN feedback matrix mixes `f0..f3` from the previous samples and the input. Once a NaN entered any line, the matrix recirculated it through every line forever. The reverb has no recovery path. Same bug class as loop 25's filter z1/z2 poisoning.

A third minor: `set_param` accepted non-finite parameter values and pushed them through fmin/fmax (which propagate NaN as NaN per IEEE rules unless `fmin`/`fmax` are the C99 forms — they are, so NaN is *suppressed* and you get the other operand). Still safer to reject up front before the room-size recompute.

**DECIDE**:
1. Move the allocation out of the audio thread: in the constructor, `lines[i].assign(MAX_DELAY_SAMPLES, 0.0f)` once (≈1.5 MB total, acceptable per-instance). `set_delay_lengths` then only updates `delay_len[i]` and `std::fill`s the active range — never reallocates.
2. Add a per-sample NaN/Inf guard on `f0..f3`: if any feedback term goes non-finite, reset all four to zero and zero the current output sample. The next sample re-enters the FDN cleanly.
3. Reject non-finite values at the `set_param` entry point (early `if (!std::isfinite(value)) return;`).

**DEVIL'S ADVOCATE**:
- *Correctness*: Pre-allocating to 96000 samples × 4 lines × 4 bytes = 1.5 MB per Reverb node is a fixed cost paid once at graph compile time, on the host thread. No audio-thread allocator interaction afterwards. The `std::fill` in `set_delay_lengths` is O(n) but lock-free and bounded — the existing `assign` was at least the same cost plus the alloc. *No standing.*
- *Scope*: Could the input to the reverb produce NaN even after loops 23/24 hardened the oscillator? Yes — any custom node, denormal cascade, or future node could. NaN guard is necessary, not symptom-treating. *No standing.*
- *Priority*: RT allocation is priority 1 (definitionally a real-time correctness bug — silent xrun). NaN poisoning is priority 1 (silent permanent corruption). *No standing.*

**ACT**:
- `cpp/include/nodes/reverb.h`: pre-allocate to `MAX_DELAY_SAMPLES`, `set_delay_lengths` switches from `assign` to `std::fill` over the active prefix, NaN scrub on feedback matrix, non-finite param rejection.
- `rust/tests/engine_smoke.rs`: `reverb_param_automation_under_load_stays_bounded` — drives 40 DELAY_TIME changes mid-stream + sends NaN/Inf rogue params, asserts every output sample finite + bounded.

Result: 38 lib + **13 smoke** + 5 ui_main = **56 tests pass**. fmt + clippy clean.


## Loop 27 — DelayNode DELAY_MODE NaN UB + delay/comb/all-pass NaN poisoning

**OBSERVE.** `cpp/include/nodes/delay.h` 376 lines. SIMPLE_DELAY,
CHORUS, FLANGER, VIBRATO, PITCH_SHIFT, REVERB modes share one
`delay_buf[MAX_DELAY_LEN=96000]` plus comb/all-pass arrays for
Schroeder reverb. No allocations on the audio thread (fixed-size
arrays, not vectors — unlike loop 26's reverb.h). Three priority-1
defects were live:

1. `DELAY_MODE: mode = static_cast<int>(value)` — same NaN→int UB as
   the FILTER_MODE bug fixed in loop 25 (filter.h). `static_cast<int>`
   on a non-finite float is undefined behavior in C++; the cast can
   produce arbitrary values including out-of-range modes that the
   subsequent switch doesn't cover (default path memcpy's unchanged,
   so a corrupt mode silently bypasses processing).
2. `process_delay` writes `in[i] + delayed * feedback` straight into
   the delay buffer with no NaN/Inf scrub. Feedback is clamped to
   [0, 0.99] so a single poisoned sample recirculates with ~1% decay
   per round — practically never recovers.
3. `process_reverb` writes `input + reverb_lp * comb_fb` into each
   comb buffer and `ap_out + buf_val * ap_g` into each all-pass with
   no scrub. Same recirculation problem; also `reverb_lp` (the IIR
   damping state) is itself updated in-loop and could pin to NaN.

**ORIENT.** Same bug class as loops 25/26. delay.h is the third-most-
used effect node in graphs (after Output/Gain and Oscillator), and is
the canonical target for parameter automation, so NaN robustness here
is high-leverage.

**DECIDE.** Three candidates:
  (a) DELAY_MODE NaN guard + clamp (1-line fix, eliminates UB).
  (b) NaN scrub in process_delay (4 lines, prevents permanent
      poisoning of the most-used mode).
  (c) NaN scrub in process_reverb comb + all-pass + reverb_lp (10
      lines, prevents permanent poisoning of the second-most-used
      mode and recovers the damping IIR).
All three are priority-1 silent-corruption fixes; bundle them.

Phaser dead code (line 316: `y = phaser_ap[s] + coeff*(tmp-ap_val);`
overwritten on line 317 by `y = ap_val;`) is a logic bug (priority
4) and the correct allpass-cascade formula needs spec lookup —
deferred to loop 28. Vibrato write-then-read off-by-one likewise
deferred.

**DEVIL.**
- *Correctness attack*: does `if (!std::isfinite(value)) return` at
  set_param entry break legitimate set_param(0.0) calls? No —
  `std::isfinite(0.0f)` is true; it only rejects NaN/±Inf, which
  have no valid parameter meaning.
- *Scope attack*: is the real bug the missing global parameter
  validation layer rather than per-node guards? Possibly, but a
  defense-in-depth per-node guard is the right level: the IPC ring
  carries raw floats from JS/Rust callers and an upstream guard
  could regress without the node noticing.
- *Priority attack*: should we have fixed phaser dead code first?
  No — phaser produces audibly wrong output but not silent corruption
  of state that propagates to other nodes. NaN in delay_buf can
  poison the mix bus and downstream nodes via the audio graph.
- *Regression risk on reverb*: zeroing comb/all-pass writes on NaN
  is a one-sample silent glitch vs permanent garbage, and only
  triggers off non-finite inputs. The unconditional `reverb_lp = 0`
  on poison resets the damping state, which is correct — the IIR
  is exactly the value that just produced NaN, so it must be reset
  too.

**ACT.**
- delay.h `set_param`: add `if (!std::isfinite(value)) return` early
  guard; clamp DELAY_MODE to `[0, PITCH_SHIFT]` after the cast.
- delay.h `process_delay`: scrub `input` and the output `y` for NaN
  before writing buffer / output.
- delay.h `process_reverb`: scrub the comb feedback term (and reset
  reverb_lp on poison), scrub each all-pass write+tmp pair, scrub
  output `y`.
- engine_smoke.rs: new test `delay_node_rejects_rogue_params_and_stays_bounded`
  — Osc(440)→Delay→Output graph, sets DELAY_MODE=NaN, DELAY_MODE=1e9,
  DELAY_FEEDBACK=Inf, DELAY_TIME=NaN at audio rate, then switches to
  reverb mode mid-stream and asserts every output sample is finite
  and ≤4.0.

**VERIFY.** `cmake --build cmake-build` → green.
`cargo test --release --test engine_smoke` → 14 passed (was 13).
`cargo fmt --all` clean. `cargo clippy --release --all-targets -- -D warnings` clean.

**NEXT.** Loop 28: phaser dead-code in delay.h `process_phaser`
(lines 311-319) — dead assignment masks broken allpass cascade.
Then loop 29: audit `cpp/include/nodes/effects.h` for NaN/RT.

## Loop 28 — Phaser dead-code allpass cascade + vibrato write-then-read off-by-one

**OBSERVE.** Two priority-4 logic bugs in `cpp/include/nodes/delay.h`
left over from the OBSERVE pass in loop 27:

1. `process_phaser` (lines ~349-356): the cascade body computed
   `ap_val = coeff*(y - phaser_ap[s]) + y; ... y = phaser_ap[s] +
   coeff*(tmp-ap_val); y = ap_val;` — the second `y` assignment
   overwrites the first immediately, so the intermediate value is
   dead code. The surviving recurrence is
   `phaser_ap[s] ← (1+coeff)·y - coeff·phaser_ap[s]` and `y` mirrors
   that — i.e. `H(z) = (1+coeff)/(1+coeff·z⁻¹)`, a 1-pole low-pass
   with magnitude response that is decidedly NOT unit-modulus. The
   "phaser" therefore had no phase-modulation behavior.
2. `process_vibrato` (lines ~381-386): wrote `delay_buf[write_pos]
   = in[i]`, advanced write_pos, then read at `(write_pos - d0)`.
   With d0=1 the read returns the just-written sample (zero delay),
   and the lerp coefficients `frac` index off-by-one for all small
   d0. Sub-perceptual at d0>10 but wrong on principle. Chorus and
   pitch-shift in the same file already use the correct
   read-before-write order.

**ORIENT.** Both bugs are in modes that have zero smoke coverage
(loop 27 only tested SIMPLE_DELAY and SCHROEDER_REVERB). Bundling
them keeps the cost of adding test scaffolding amortized.

**DECIDE.** Replace phaser body with canonical Direct-Form-II
1st-order all-pass: `v = x - coeff·s; y = coeff·v + s; s = v`,
H(z) = (coeff + z⁻¹)/(1 + coeff·z⁻¹), `|H(e^jω)| = 1`. Add NaN
scrub on output. Swap vibrato to read-before-write; add NaN scrub
on output.

**DEVIL.**
- *Correctness*: New phaser formula's allpass property
  `|H(e^jω)|² = (coeff² + 2·coeff·cos(ω) + 1) / (1 + 2·coeff·cos(ω)
  + coeff²) = 1` — verified on paper before committing.
- *Scope*: Real fix could alternatively swap to a Direct-Form-I
  allpass with separate x/y state per stage. DF-II with single state
  per stage is canonical, requires no new fields, and matches the
  existing `phaser_ap[s]` shape — least-disruptive correct fix.
- *Priority*: Phaser bug is audibly wrong; vibrato is hygiene. Could
  have split into separate loops; bundling is justified by shared
  test scaffolding and same-file scope.
- *Regression*: lint passed, full smoke suite passes (15/15). The new
  test asserts non-trivial energy through the phaser
  (`max_abs > 0.05`) so a broken collapse-to-zero formula would
  fail the test rather than silently pass.

**ACT.**
- delay.h `process_phaser`: replace dead-code body with DF-II
  allpass per stage; add isfinite scrub on combined output.
- delay.h `process_vibrato`: swap to read-before-write order
  (matches chorus/pitch ordering); add isfinite scrub on output.
- engine_smoke.rs new test
  `delay_phaser_and_vibrato_modes_stay_bounded`: PHASER mode (=4)
  with stages=6, depth=1.0, asserts every sample finite + bounded
  + non-trivially loud (>0.05). VIBRATO mode (=5) with same LFO,
  asserts finite + bounded.

**VERIFY.** cmake build clean. cargo test 15/15 pass (was 14).
fmt + clippy --release --all-targets -- -D warnings clean.

**NEXT.** Loop 29: audit `cpp/include/nodes/effects.h` for the same
NaN/UB/RT patterns (FILTER_MODE-style cast UB on EFFECT_MODE,
unbounded distortion drive recursion, bitcrusher rate-reduction
counter overflow, missing waveshaper input clamp).

## Loop 29 — EffectsNode EFFECTS_MODE NaN UB + tone_lp/crush_held NaN poisoning

**OBSERVE.** `cpp/include/nodes/effects.h` 296 lines, 7 modes
(DISTORTION/OVERDRIVE/BITCRUSHER/RING_MOD/WAVESHAPER/TREMOLO/
STEREO_WIDENER). Audit found three priority-1 silent-corruption
defects mirroring loops 25/27:

1. `EFFECTS_MODE: mode = static_cast<int>(value)` (line 76) — NaN→int
   UB, plus the process() `default:` arm silently bypasses processing
   for out-of-range mode values.
2. distortion/overdrive update `tone_lp` IIR state with no NaN
   scrub: `tone_lp += tone*(distorted - tone_lp)` permanently
   poisons tone_lp if `distorted` (= tanh(NaN) or exp(NaN))
   produces NaN. Distortion is arguably the most-used effect mode.
3. bitcrusher's `crush_held` is a sample-and-hold register; with
   sample_rate_reduce ≤ 100, the held value can persist for 100
   samples between updates. NaN input freezes crush_held at NaN
   for that entire window.

Also spotted (deferred to loop 30, priority 4): overdrive uses
raw `tone_lp` instead of the tone-blended `tone_lp*tone +
distorted*(1-tone)` that distortion uses. At tone=0 the overdrive
wet path silences entirely because tone_lp never updates.

**ORIENT.** Same defense-in-depth pattern as loops 25-28; cost is
one isfinite per sample on the affected paths plus one branch at
set_param. Cover all 7 modes in one regression test since none
had any smoke coverage prior.

**DECIDE.** Bundle items 1-3 (priority-1 NaN/UB family). Defer
overdrive tone-blend bug to loop 30 alongside other effects.h
logic-bug cleanup (process_widener is a comb filter not an
allpass — variable name lies).

**DEVIL.**
- *Correctness*: `if (!std::isfinite(value)) return` doesn't reject
  legitimate 0.0 params (`isfinite(0.0)` is true). EFFECTS_MODE
  clamp `[DISTORTION=0, STEREO_WIDENER=6]` matches the actual
  process() switch arms.
- *Scope*: ring_mod / tremolo phase accumulators were NOT scrubbed
  — they're inherently bounded by the wrap-to-TWO_PI logic and
  `ring_freq`/`trem_rate` clamps prevent unbounded increments. The
  scope is correct: only state with multiplicative feedback
  (tone_lp's +=, crush_held's persistent register) needs NaN scrub.
- *Priority*: Bitcrusher is rarely user-driven from the UI but is
  a stress-test target for fuzzing — covering it costs <5 lines.
- *Regression*: full smoke suite green (16/16). New test cycles all
  7 modes after sending NaN/Inf to EFFECTS_MODE/DRIVE/BIT_DEPTH/
  RATE_REDUCE; if any mode produces non-finite output the test
  fails by mode name.

**ACT.**
- effects.h `set_param`: isfinite early-return; clamp EFFECTS_MODE
  to [0, STEREO_WIDENER].
- effects.h `process_distortion`: isfinite scrub on `distorted`,
  on `tone_lp` after the IIR update, and on the final `y`.
- effects.h `process_overdrive`: same isfinite scrubs (distorted,
  tone_lp, y).
- effects.h `process_bitcrusher`: scrub input before
  round/quantize, scrub crush_held after, scrub final y.
- engine_smoke.rs new test
  `effects_node_all_modes_stay_bounded_under_param_storm`: Osc(440)→
  Effects→Output, sends NaN/Inf params, then iterates all 7 modes
  and asserts every output sample is finite + ≤4.0 with the
  mode name in the failure message.

**VERIFY.** cmake build clean. cargo test 16/16 (was 15). fmt +
clippy --release --all-targets -- -D warnings clean.

**NEXT.** Loop 30: overdrive tone-blend bug (process_overdrive
uses raw tone_lp instead of `tone_lp*tone + distorted*(1-tone)`,
silencing at tone=0). Plus possibly process_widener variable
naming (`ap_buf`/`ap_pos` for a comb-filter delay line — misleading)
or gain.h audit. Decide based on what re-OBSERVE shows.

## Loop 30 — GainNode GAIN_MODE NaN UB + env_db Inf-pin + simple-gain scrub

**OBSERVE.** `cpp/include/nodes/gain.h` 218 lines, 5 modes
(SIMPLE_GAIN/COMPRESSOR/LIMITER/GATE/EXPANDER). Audit found:

1. `GAIN_MODE: mode = static_cast<int>(value)` (line 57) — same
   NaN UB as filter/delay/effects (loops 25/27/29).
2. `process_compressor` and `process_gate` envelope follower:
   `env_db = coeff * env_db + (1-coeff) * input_db`. If env_db
   ever becomes ±Inf (e.g. an upstream chain hop produces Inf
   before its own scrub kicks in next loop iteration), both attack
   and release branches preserve Inf forever (`coeff*Inf +
   (1-coeff)*finite = Inf`). Then `db_to_lin(-Inf) = 0` and
   output is permanently silent. Defense-in-depth gap.
3. Simple-gain smoothing: `gain = gain*0.99 + target_gain*0.01`
   with no scrub. Although `target_gain` is NaN-safe via
   fmin/fmax (which suppress NaN), `gain` could still go non-
   finite under FP arithmetic edge cases — defense is cheap.

Re-prioritized loop 30 from the previously-planned overdrive tone
bug (priority 4) to gain.h hardening (priority 1) per the loop
priority ordering.

**ORIENT.** Same defense-in-depth pattern as loops 25-29. The
env_db Inf-pin is theoretically gated behind upstream scrubs but
the pattern is symmetric with the other IIR fixes.

**DECIDE.** Bundle: isfinite param guard + GAIN_MODE clamp +
env_db clamp+scrub in both compressor/gate branches + simple-gain
state scrub + per-sample output scrub in all three process
functions.

**DEVIL.**
- *Correctness*: env_db clamp `[-200, +200]` dB is generous (room
  dB doesn't exceed ±150 in realistic signals); 200 dB > 0 dBFS
  by 200 dB, so no compression curve will reach that bound from
  legitimate audio.
- *Scope*: I considered the deferred overdrive tone bug from loop
  29 — but priority ordering puts gain.h's MODE NaN UB at priority 1
  vs overdrive at priority 4. Take the priority-1 fix first.
- *Priority*: GAIN_MODE NaN UB is unconditional UB; cost of the fix
  is a single isfinite check and a clamp.
- *Regression*: `gain` reset path (`if (!isfinite(gain)) gain =
  target_gain`) — target_gain is itself NaN-safe so this can't
  re-poison. Loop test with rogue ATTACK = -Inf verifies env_db
  doesn't poison even when attack_coeff = exp(-1/(NegInf*sr*0.001))
  = exp(0) = 1, which makes env_db never decay (still bounded).

**ACT.**
- gain.h `set_param`: isfinite early-return; clamp GAIN_MODE to
  `[SIMPLE_GAIN, EXPANDER]`.
- gain.h `process_compressor`/`process_gate`: clamp env_db to
  [-200, +200] dB and scrub NaN explicitly after each IIR update;
  scrub final `buf[i]*gain_lin` output for non-finite before
  store-back.
- gain.h simple-gain default arm: scrub `gain` IIR state (reset to
  target_gain on poison) and scrub output sample.
- engine_smoke.rs new test
  `gain_node_all_modes_stay_bounded_under_param_storm`: Osc(440)→
  Gain→Output, sends NaN/Inf to GAIN_MODE/GAIN_LEVEL/THRESHOLD/
  ATTACK, then iterates all 5 modes and asserts every sample is
  finite + ≤12.0 with the mode name in failure messages.

**VERIFY.** cmake clean. cargo test 17/17 (was 16). fmt + clippy
--release --all-targets -- -D warnings clean.

**NEXT.** Loop 31: overdrive tone-blend logic bug in
`cpp/include/nodes/effects.h` `process_overdrive` (priority 4 —
re-deferred from loop 29 by loop 30's priority-1 win). Plus
`process_widener` is misnamed (uses `ap_buf` for a comb-filter
delay line, not an actual all-pass) — rename or rewrite to a
real allpass.

## Loop 31 — Final-stage NaN scrub + ±1.0 hard clamp on audio-engine ring write

**OBSERVE.** Re-OBSERVE turned up a higher-priority defect than the
loop-30 next.md plan (overdrive tone-blend, priority 4). At
`cpp/src/audio_engine.cpp:212-213` the per-block output-ring write
loop did a raw `e->output_ring_buffer[(oh + i) & (cap - 1)] =
src[i]` — no NaN/Inf scrub, no clamp. This is the LAST stage
before samples cross into Rust and then into the cpal DAC
callback. Every per-node scrub added in loops 23-30 protects
against in-graph corruption, but a single regression in any node
(or a future-added node that forgets the scrub) would leak
non-finite samples directly to the audio device. Best case is a
click; worst case is a driver fault or sound-server hang.

Also confirmed there is NO standalone OutputNode class —
`NODE_TYPE_OUTPUT` aliases to GainNode in `create_node()`
(audio_engine.cpp:97-99). The "output" is the ring write itself.

**ORIENT.** Priority-1: silent corruption / data corruption at
the device boundary. The cost of the fix is one isfinite + two
compare-and-clamp per sample (≈ 256 × ~3 cycles / block ≈ 800
cycles, negligible against the rest of the graph). This is
precisely the "things that crash or corrupt silently" priority-1
slot, applied at the place where it matters most.

**DECIDE.** Hard clamp the ring output to ±1.0 with NaN/Inf →
0.0. Hard rather than soft (tanh) because:
  (a) hard clamp is exactly what the DAC does anyway,
  (b) tanh costs ~30× more CPU,
  (c) graph-level nodes are already amplitude-bounded near unity;
      a clamp event indicates upstream regression, not normal
      signal levels.

**DEVIL.**
- *Correctness*: ±1.0 maps to the DAC's full-scale range (cpal's
  f32 output expects [-1, 1]). A graph that wants > 0 dBFS is
  already misconfigured. The clamp is correct.
- *Scope*: I considered also adding a soft-knee compressor at the
  boundary, but that's a feature (a "master limiter"), not a
  defense. The defense is the clamp; the limiter would be a
  separate node-type RFE.
- *Priority*: This is the audio-output equivalent of the parser
  layer in a network protocol — last validation before
  untrusted data hits OS-level code. Definitely priority 1.
- *Regression*: Existing test bounds are `≤ 4.0` (gain), `≤ 1.0`
  for OscNode-only tests. Verified via grep that only one
  `max_abs >` floor assertion exists (phaser, threshold 0.05) and
  it sits comfortably inside ±1.0. New test
  `engine_final_stage_clamps_output_to_unity` would have failed
  on the old behavior (max_abs would have been ~10.0 after
  smoothing).

**ACT.**
- audio_engine.cpp ring write: replace single-line copy with
  isfinite-check + ±1.0 clamp + commented rationale.
- engine_smoke.rs new test forces 10× simple gain on a 440 Hz
  oscillator and asserts every ring sample is finite, ≤ 1.0+ε,
  and that gain saturation actually triggered (max_abs > 0.95).

**VERIFY.** cmake clean. cargo test 18/18 (was 17). fmt + clippy
--release --all-targets -- -D warnings clean.

**NEXT.** Loop 32: re-prioritize from `next.md`. Highest-leverage
remaining priority-1 candidates:
  (a) OscillatorNode `process()` writes to scratch with no per-
      sample isfinite check — extreme FM/AM intermodulation could
      still produce inf/nan that the loop-23 wrap-fix doesn't
      catch (e.g. AMP × tanh(NaN_input)).
  (b) Look at `audio_engine.cpp` for other RT-discipline holes
      (allocations, mutexes, syscall paths).
  (c) Then drop to priority-4: overdrive tone-blend bug.

## Loop 32 — Oscillator NaN/Inf param hardening + rolloff clamp
**OBSERVE**: oscillator.h was missed in loops 23-24 hardening pass — no isfinite guard at set_param entry, `waveform = static_cast<int>(value)` is NaN→int UB, and `rolloff = value;` totally unclamped. Combined with `amp /= (1.0f + rolloff)` at the additive synth (line 258), rolloff=-1 is divide-by-zero, rolloff<-1 is sign-flipping Inf-pump.
**ORIENT**: priority-1 silent corruption — UI knob updates can deliver any f32 value, including NaN/Inf, and the additive waveform is reachable via WAVEFORM_TYPE=12.
**DECIDE**: add the canonical defense pattern (isfinite early-return + WAVEFORM_TYPE clamp) and clamp rolloff to [0, 8].
**DEVIL**:
  - *Correctness*: clamping rolloff to [0, 8] forecloses negative rolloff curves — but negative rolloff would amplify higher harmonics in additive synth, which in combination with wide harmonic counts would explode amplitude even before reaching -1; positive-only is the only safe range.
  - *Scope*: are there other oscillator state vars unclamped? Reviewed — frequency/detune/FM/AM/sub_level were already clamped in loops 23-24. rolloff was the only survivor.
  - *Priority*: deferred overdrive tone-blend bug again. Justified — it's priority-4 and this is priority-1.
**ACT**: oscillator.h: isfinite early-return at set_param; WAVEFORM_TYPE clamped to [0, ADDITIVE=12]; rolloff clamped to [0.0, 8.0]. Added smoke test `oscillator_node_rejects_rogue_params_and_rolloff` that pumps NaN/Inf into WAVEFORM_TYPE/OSC_FREQUENCY/rolloff while holding ADDITIVE waveform, then cycles all 13 waveforms — asserts every sample finite and ≤1.0+ε after the final-stage clamp.
**Result**: 19/19 smoke tests pass, clippy clean.

## Loop 33 — Overdrive tone-blend bug
**OBSERVE**: process_overdrive in effects.h fed `tone_lp * distort_mix` into the wet path. process_distortion (the canonical reference for the same control set) does `shaped = tone_lp*tone + distorted*(1-tone)` then mixes shaped into wet. The IIR coeff for tone_lp is `tone` itself, so at tone=0 tone_lp NEVER updates (stays at its 0.0 init), and at distort_mix=1.0 the wet path is identically silent — overdrive bypasses to dry input at every distort_mix<1, and to silence at 1.
**ORIENT**: priority-4 logic bug — wrong output for valid input on a documented user-facing control. Deferred since loop 29; no priority-1/2/3 surfaces remain unaddressed in nodes audited so far.
**DECIDE**: mirror process_distortion's blend exactly. Add `shaped = tone_lp*tone + distorted*(1-tone)` and mix `shaped` instead of `tone_lp`.
**DEVIL**:
  - *Correctness*: the new blend at tone=1 collapses to `tone_lp`, matching the old behavior exactly there — backward-compat preserved at the bright extreme. At tone=0 it produces the raw distorted signal, which is the expected "bright/no-tone-rolloff" position. Coefficient pinning at tone=1 still reaches steady state of distorted via the `tone_lp += 1.0*(distorted - tone_lp)` recurrence (one-block convergence).
  - *Scope*: are other modes affected? Only distortion (already correct) and overdrive share tone_lp. No cross-contamination.
  - *Priority*: loop 32 closed the last priority-1 surface in primary nodes. This is now the highest-leverage outstanding correctness item.
**ACT**: edited overdrive blend to match distortion. Added smoke test that drives a 440Hz sine at drive=8, distort_mix=1.0, tone=0 and asserts RMS>0.05 (pre-fix RMS would be ~0). Bounded by final-stage clamp.
**Result**: 20/20 smoke tests pass, clippy clean.

## Loop 34 — Surface set_param errors in start_engine
**OBSERVE**: tauri-ui/src-tauri/src/main.rs:184,196 wrapped `engine.set_param(n.id, ...)` with `let _ = ...`. The only failure mode of set_param is "param queue full" (cap 8192). The audio thread that drains the queue has NOT started yet at this point in start_engine (line 204), so any large graph that pushes >8192 params during init silently drops the overflow — leaving C++ nodes initialised with their compile-time defaults instead of the user-supplied values. UI shows "engine started" with no indication of dropped params.
**ORIENT**: priority-2 missing error handling on a critical path. The user has no way to detect that their saved patch was partially loaded.
**DECIDE**: replace the silent swallows with `?` propagation, formatting a structured error message that includes the offending node id and param hash.
**DEVIL**:
  - *Correctness*: could surface false positives if start_engine is ever called on a graph that previously worked? Currently param queue capacity 8192 is far above realistic graph size (256 nodes × 5 params = 1280), so this would only fire on a genuine bug — a queue corruption or runaway init loop. Failing loud is correct.
  - *Scope*: only the start_engine path; the live `set_param` Tauri command (line 222+) already returns Result<(), String>. No symptom-vs-cause confusion — root cause is the bare `let _`.
  - *Priority*: midi_input bounds and graph cycle edge cases also exist, but those have different blast radius. This is the highest-leverage error-handling gap on the load-patch flow.
**ACT**: replaced both `let _ = engine.set_param(...)` lines with `engine.set_param(...).map_err(|e| format!(...))?` carrying node id and param hash for diagnosability.
**Result**: full workspace builds clean. 20/20 engine_smoke tests pass. shadow_graph + lockfree_queue tests pass. clippy clean.

## Loop 35 — Audio thread SPSC memory ordering
**OBSERVE**: cpp/src/audio_engine.cpp had two `memory_order_acquire` loads on consumer-/producer-OWN indices:
  - line 153 `param_queue_tail` (consumer-own) loaded Acquire — should be Relaxed.
  - line 204 `output_ring_head` (producer-own) loaded Acquire — should be Relaxed.
The Rust counterpart in rust/src/lockfree_queue.rs uses the canonical pattern: own=Relaxed, remote=Acquire, publish=Release. C++ was the outlier.
**ORIENT**: not a correctness bug on x86 (Acquire is a no-op under TSO) but a real wasted barrier on Apple Silicon / ARM where Acquire maps to `ldar`. Two extra barriers per audio block × ~187 blocks/sec at 256 samples / 48kHz = ~374 wasted barriers/sec on the realtime thread.
**DECIDE**: change own-index loads to Relaxed.
**DEVIL**:
  - *Correctness*: Acquire on a single-thread-owned variable is identical in semantics to Relaxed for that thread (program order subsumes any synchronization). Cross-thread, the consumer never reads the producer's own-index — only its publish-side load via the *remote* index, which is correctly Acquire. No regression possible.
  - *Scope*: are there other ordering inconsistencies? `is_running.load(acquire)` at line 146 is correct (synchronizes with the main thread's Release stop). `sample_count.fetch_add(release)` at line 199 is over-strong if no consumer reads with Acquire, but correct, and not on a queue path. Keep them.
  - *Priority*: priority-5 (performance cliff on a realtime thread). Not blocking, but cheap to fix and aligns with the documented SPSC contract — closes a divergence that would otherwise confuse anyone porting the engine to a new platform.
**ACT**: relaxed both own-index loads. Commented the rationale inline so future readers see the SPSC contract.
**Result**: 20/20 engine_smoke pass, fmt clean, clippy clean.

## Loop 36 — cpal stream broken on stereo-default devices
**OBSERVE**: tauri-ui/src-tauri/src/main.rs:99-103 hard-coded `channels: 1`. On most desktop audio backends the default device is stereo (often the only supported channel count). On WASAPI / pure ALSA this would have failed `build_output_stream` with "format not supported" — a startup error users likely hit. On backends that *did* accept mono (PipeWire, CoreAudio) the callback received a buffer sized in mono frames but had no fan-out, so the ring read was correct only for genuinely mono devices. The deeper bug if cpal ever transparently up-mixed: `ring.read(buffer)` reads `buffer.len()` frames out of the mono ring, so a stereo buffer would consume the engine's output at 2× speed.
**ORIENT**: priority-2 (configuration broken on common hardware, no diagnostic). The whole shipped Tauri shell only works correctly if the OS happens to expose a mono default device.
**DECIDE**: query `device.default_output_config()` for the actual channel count; allocate a per-callback mono scratch buffer; fan the mono signal out across all channels.
**DEVIL**:
  - *Correctness*: heap-allocating `mono_scratch` lazily inside the realtime cpal callback is normally a smell, but the alloc only happens on the very first call (or on a callback-size growth, which cpal never triggers in practice with `BufferSize::Default`). `Vec::resize` to a value it already exceeds is a no-op. Could pre-size in `open_cpal_stream` if cpal exposed `default_output_config().buffer_size`, but the default is opaque. Acceptable trade-off; commented inline.
  - *Scope*: the headless `rust/src/main.rs` test harness has no cpal — out of scope.
  - *Priority*: priority-2 fix. Above the deferred MIDI/UI items.
**ACT**: rewrote `open_cpal_stream` to honour the device's default channel count. Added stderr log of channel count + sample rate so users can see what device opened.
**Result**: workspace builds clean, 20/20 engine_smoke pass, all crate tests pass, clippy clean.

## Loop 37 — MIDI parser data-byte high-bit masking
**OBSERVE**: rust/src/midi_input.rs::parse cast `msg[1]`, `msg[2]` to u32 directly without masking. MIDI 1.0 requires data bytes to have bit 7 clear (range 0–127), but USB-MIDI bridges and buggy device firmware can emit data bytes with bit 7 set. Without a mask:
  - NoteOn pitch could exceed 127 → if any future C++ MIDI consumer uses pitch as an array index without bounds-checking, OOB read.
  - PitchBend `(d2 << 7) | d1` would spill the d1 high bit into the d2-shift region, producing a corrupted 14-bit value.
**ORIENT**: priority-7 (defense-in-depth against malformed input on a path that doesn't currently corrupt anything because MIDI isn't drained yet, but will the moment a MIDI consumer node lands). Cheap and safe.
**DECIDE**: pre-mask both data bytes with `& 0x7F` before all parser arms.
**DEVIL**:
  - *Correctness*: masking can never make a spec-compliant message wrong (the high bit is required to be 0 already). Tests `parse_pitch_bend_packs_14_bits_lsb_first` and the existing channel-nibble test still pass.
  - *Scope*: do we need to validate the status byte too? `msg[0] & 0xF0` already strips the channel nibble. Status validation is implicit in the `match` arms.
  - *Priority*: lower than DSP/host issues, but the ones above it are larger feature-gap work (MIDI consumer integration). One of the better-scoped quick wins still remaining.
**ACT**: extracted `d1`, `d2` from `msg.get(N).copied().unwrap_or(0) & 0x7F` (this also makes the indexing infallible, so a panic from misaligned message length is no longer possible — though the `3..` arm already guarded that). Added regression test `parse_strips_high_bit_on_malformed_data_bytes`.
**Result**: 10/10 midi parse tests pass, 39/39 lib unit tests pass, 20/20 engine_smoke pass, clippy clean.
