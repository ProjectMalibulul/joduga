# Loop 12 candidate: C++ static_assert mirror of FFI offsets

Loop 10/11 pinned the layout from the Rust side. Add matching
static_assert(offsetof(...) == N, ...) lines in cpp/include/audio_engine.h
so a C++-side reorder fails at compile time on the C++ side too —
not just when someone happens to run cargo test. Closes the loop.

Backup: extract duplicate resolve_output_node_id helpers (egui +
tauri) into a shared joduga module.
