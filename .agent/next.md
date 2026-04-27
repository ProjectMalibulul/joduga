# Loop 20 candidate: assert cpu_load_permil populates under load

StatusRegister.cpu_load_permil is exposed via
audio_engine_wrapper::cpu_load_permil() but no test verifies that the
C++ engine actually populates it. Build a smoke test with a heavier
graph (e.g. 3 oscillators + filter + reverb if available) and assert
that after a few seconds of processing the field is > 0 and < 1000.
Skip on CI runners where SCHED_FIFO note appears (already flagged in
existing smoke tests).

Backup loop 20: rate-limit the [midi] queue full log added in loop 19;
or migrate it to a status_register counter if the FFI ABI bump is
acceptable.

Backup loop 21: enum-keyed BuiltinTemplate (carryover).

Backup loop 22: audit cpp/include/nodes/*.h for the same num_inputs/
num_outputs descriptor-vs-node disagreements that loop 6 caught for
GainNode. Spot-check ReverbNode, DelayNode, EffectsNode.
