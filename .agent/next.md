# Loop 17 candidate: enum-keyed BuiltinTemplate (deeper fix from loop 13)

The string-name lookups in JodugaApp::new() are still fragile. Add a
stable enum BuiltinTemplate { SineOscillator, LowPassFilter, Gain,
SpeakerOutput, ... } used as the catalog key, with the name field
remaining as the user-facing label. JodugaApp::new() then looks up by
enum variant and silent renames are impossible.

Backup: shadow_graph.rs::add_node currently doesn't validate that the
output_node_id specified in ShadowGraph::new actually corresponds to
an Output-type node when it's added. A user could pass an Oscillator
node with id == output_node_id and the engine would receive a
non-Output node as the sink. Audit and add validation.
