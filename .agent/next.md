# Loop 11 candidate: ParamUpdateCmd / MIDIEventCmd / StatusRegister offset_of

Loop 1 pinned alignment of ParamUpdateCmd and MIDIEventCmd but didn't
pin field offsets. Add offset_of! tests for those structs and for
StatusRegister (loop 10's ABI test family didn't cover the
lockfree_queue cmd structs because they live in a different module).

Backup candidate: extract the duplicate resolve_output_node_id helpers
into a shared module — paramaterised by a small trait so both call
sites can reuse it.
