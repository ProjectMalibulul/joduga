# Loop 19 candidate: audit the MIDI input path

rust/src/midi_input.rs has not been touched by the agent loop yet.
It feeds the lock-free MIDI queue that the C++ engine drains every
block. Likely concerns to audit, in priority order:
1. Does the MIDI parser handle malformed running-status messages?
2. Is the queue write path lossy (drops on full) or back-pressured?
   If lossy, is there a status_register counter for dropped events?
3. Are unit tests covering parse → queue → drain end-to-end?

Backup loop 19: enum-keyed BuiltinTemplate (carryover from loop 18 doc).

Backup loop 20: assert cpu_load_permil advances under a heavy graph
in a smoke test (currently the field exists in StatusRegister but no
test verifies the C++ side actually populates it).
