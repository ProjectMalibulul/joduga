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
