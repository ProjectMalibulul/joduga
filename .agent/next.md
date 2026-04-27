# Loop 18 candidate: enum-keyed BuiltinTemplate (deeper fix from loop 13)

The string-name lookups in JodugaApp::new() are still fragile. Add a
stable enum BuiltinTemplate { SineOscillator, LowPassFilter, Gain,
SpeakerOutput, ... } used as the catalog key, with the name field
remaining as the user-facing label.

Backup: audit cpp/src/audio_engine.cpp for malformed-input handling on
audio_engine_init — pointer args nullable? Lengths zero? Pre-loop-5
behavior when num_outputs=0 was ambiguous. Worth a defensive pass.
