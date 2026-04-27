# Loop 15 candidate: enum-keyed catalog (deeper fix from loop 13)

The string-name lookups in JodugaApp::new() and elsewhere are fragile.
Add a stable enum (BuiltinTemplate::SineOscillator etc.) used as the
catalog key, with name-string lookup remaining for serialization. Then
demo-graph construction can never silently miss a renamed template
because the enum variant ties source code to the catalog at compile
time.

Backup: cpu_load_permil isn't asserted to advance in engine_smoke.rs.
Add a multi-node graph variant of the smoke test that drives the
status register past 0.
