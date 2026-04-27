# Loop 22 candidate: graph_version_ref staleness watchdog

The C++ engine ticks status_register.graph_version every block (~187
Hz). The Rust UI never polls it for a stall. If the audio thread hangs
(deadlock, infinite loop in a node), nothing surfaces it: the UI keeps
running, leaving the user with silent or stuck audio.

Plan: add a wrapper helper `is_audio_thread_alive(timeout)` that snaps
the value, sleeps `timeout`, and returns whether it advanced. Add a
smoke test that exercises the happy case. (Ditto for the unhappy case
if we can synthesize a hang — likely skip given constraints.)

Backup loop 22: rate-limit the [midi] queue full log added in loop 19.

Backup loop 23: enum-keyed BuiltinTemplate (carryover).

Backup loop 24: in audio_engine_destroy, after stop()/join, ensure
status_register pointer is cleared so any racing UI read gets a stable
zero rather than a UAF. Currently status_register is a raw pointer
into Rust-owned memory; if Rust drops the wrapper while C++ is still
mid-block, there's a window where the C++ block update writes to freed
memory. Mitigated by Drop order in the wrapper but worth verifying.
