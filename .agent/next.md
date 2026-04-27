# Next loop seed (loop 28)

## Target
`cpp/include/nodes/delay.h` `process_phaser` (around lines 311-320) —
the dead-code overwrite makes the phaser a no-op (or worse, a wrong
single-stage IIR) instead of a true cascaded allpass.

```cpp
float ap_val = coeff * (y - phaser_ap[s]) + y;
float tmp    = y;
y = phaser_ap[s] + coeff * (tmp - ap_val);   // <-- DEAD: overwritten by next line
y = ap_val;                                  // <-- this is what actually runs
phaser_ap[s] = ap_val;
```

The intended formula is a 1st-order allpass per stage: standard form
is `y = -coeff*x + (x - coeff*y_prev) * something` — needs lookup
against e.g. JOS DAFX phaser reference. Most likely what was meant:
```
y_new       = -coeff * y + phaser_ap[s] + coeff * y;   // allpass output
phaser_ap[s] = y + coeff * (in - y_new);               // state update
y = y_new;
```
Verify against a known phaser implementation before committing.

## Backup target
If phaser fix is too speculative without a reference, instead fix
`process_vibrato` write-then-read ordering (line ~343): the write to
`delay_buf[write_pos]` happens BEFORE the read at `write_pos - d0`,
so when `d0 == 1` the read returns the just-written sample (zero
delay) instead of the previous one. Swap to read-before-write.

## After loop 28
Loop 29: audit `cpp/include/nodes/effects.h` for the same NaN/UB/RT
patterns. Distortion + bitcrusher + waveshaper are likely candidates
for unbounded-state bugs.
