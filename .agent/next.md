# Loop 10 candidate: ABI-layout tests for NodeDesc / NodeConnection / CompiledGraph

The lockfree_queue cmd structs got an alignment test in loop 1 but
NodeDesc, NodeConnection, AudioEngineConfig, and CompiledGraph are also
shared with C++ via FFI and have no test pinning their layout. A C++
field reorder or Rust struct edit would silently break engine init
(the new smoke test from loop 9 might catch some cases but not
field-reorder bugs that still happen to give "valid" data).

Approach:
- size_of and align_of asserts for each FFI struct.
- offset_of asserts for each field (use std::mem::offset_of! — stable
  in 1.77+).
- Mirror with constexpr/static_assert on the C++ side if possible.

Backup candidate: extract the duplicate resolve_output_node_id helpers
into a shared module.
