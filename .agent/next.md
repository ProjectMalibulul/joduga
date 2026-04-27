# Loop 21 candidate: audit cpp/include/nodes/*.h for descriptor-vs-node mismatches

Loop 6 caught GainNode declaring num_outputs=1 in C++ while the
NodeDesc descriptor said num_outputs=0. Likely sibling bugs in
ReverbNode, DelayNode, EffectsNode where C++ ctor sets num_inputs/
num_outputs but the JodugaApp catalog templates may declare different
values. Audit by:
1. Reading num_inputs/num_outputs in each cpp/include/nodes/*.h ctor.
2. Greping rust/src/ui_main.rs catalog for matching node_type values.
3. Asserting they agree, or aligning them.

Backup loop 21: rate-limit [midi] queue full log; or migrate it to a
new status_register field if the FFI ABI bump is acceptable.

Backup loop 22: enum-keyed BuiltinTemplate.

Backup loop 23: extend cpu_load_permil_advances_under_load to also
verify graph_version_ref increments — currently nothing tests that.
