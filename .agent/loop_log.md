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
