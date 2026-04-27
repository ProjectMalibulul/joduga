# Loop 23 candidate: audit cpp/include/nodes/oscillator.h for parameter handling

OscillatorNode is the node every smoke test exercises. A regression
there would silently corrupt every test simultaneously. Audit the
process() implementation for:
1. Phase accumulator wraparound (does it use modf or unbounded?)
2. WAVEFORM_TYPE bounds checking — does an out-of-range subtype
   default to silence or crash?
3. Parameter smoothing on FREQ — currently abrupt? smoothed? how
   does it sound on a sweep?
4. NoteOn handling — does the oscillator pull from MIDI or only
   from explicit Frequency param? (Test gap?)

Backup loop 23: rate-limit the [midi] queue full log added in loop 19.

Backup loop 24: enum-keyed BuiltinTemplate (carryover).

Backup loop 25: add an integration test for loop 19's NoteOn vel=0
fix that runs end-to-end through the C++ engine (i.e. verify a
synthesizer node releases the note when fed a vel=0 event).
