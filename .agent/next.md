# Loop 16 candidate: enum-keyed catalog (deeper fix from loop 13)

The string-name lookups in JodugaApp::new() and elsewhere are fragile.
Add a stable enum (BuiltinTemplate::SineOscillator etc.) used as the
catalog key, with name-string lookup remaining for serialization. Then
demo-graph construction can never silently miss a renamed template
because the enum variant ties source code to the catalog at compile
time.

Backup: extend the Osc->Output smoke test to cover FilterNode params
(FILTER_CUTOFF moves spectral content) — completes one more dispatch
pathway.
