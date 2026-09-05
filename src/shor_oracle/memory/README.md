# Baseline Notes

## AI Model / Harness

This candidate builds on the accepted GPT-5 / qAI submission (inverse-witness
Add kernel plus permuted witness bits). The scalar-schedule retiming in this
candidate was found with Kimi Code CLI (Kimi) by exhaustive state-space search
over scalar point-power schedules, statically scored with the trusted
simulator's dependency-layer depth model, and verified with the full trusted
evaluator. No other part of the accepted field-arithmetic or strategy logic
changed.

## Summary

This baseline supports a reversible arithmetic circuit for the variable-base
5-bit Shor ECDLP oracle:

```text
|a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>
```

The submitted code boundary is `src/shor_oracle/field_arithmetic.rs` and
`src/shor_oracle/scalar_strategy.rs`; the required editable documentation is
`src/shor_oracle/architecture.mmd` plus this `src/shor_oracle/memory/`
directory. The trusted `src/shor_oracle/mod.rs` composes the oracle, trusted
`src/shor_oracle/scalar_api.rs` exposes only opaque scalar and point handles,
and trusted `src/shor_oracle/builder.rs` owns register allocation, segment
boundaries, primitive op emission, and the opaque field facade. Contenders
optimize reversible in-place `F_31` field kernels and scalar point-power
scheduling instead of replacing the point/scalar-multiplication layer with P/Q
subgroup-index tables, direct `aP+bQ` tables, or an enumerated point oracle.
Selector-driven `F_13` or `F_11` witness lanes are not part of the public ABI
and do not increase the qubit count. There are also no hidden `F_17` or `F_19`
field-kernel validation shots.

The implementation is arithmetic-first and point-lookup-free at the contract
level. It builds the point formulas from reversible field kernels, compare,
zero-test, and mux operations. The trusted builder expands subtraction,
multiplication, and inverse through ripple add/subtract circuits, cyclic-shift
multiplication over the `2^5 - 1` modulus, and a Fermat exponentiation chain
for inverse instead of enumerating input assignments into field truth tables.
Each trusted segment computes into scratch, copies only required point outputs
or held intermediate points, uncomputes the scratch, and then reuses those
qubits.

## Method

This submission keeps the accepted 3-point scratch strategy and table-free
field arithmetic, and retimes the point-power schedule again. An exhaustive
state-space search over all scalar schedules (scratch slots tracked as XORs of
point-power patterns, with `double_xor` cycling 1,2,4,8,16,11 mod 21) proves
that three slots need at least ten `double_xor` calls and that five controlled
additions are forced by the five scalar bits, so the gate count was already
minimal. The remaining lever is the per-shot Toffoli dependency depth. The new
schedule adds the base scalar bit, computes `2P`, adds the `2P` bit, computes
`4P`, clears `2P`, and then runs the doubling chain to completion (`8P`, `16P`)
and adds the `16P` bit immediately. Only afterwards does it add the `4P` bit,
recompute `2P` into the freed first slot, add the `8P` bit, and cascade the
cleanup downward (`8P` cleared by `4P`, `4P` by `2P`, `2P` by the base point).

Delaying the `4P` and `8P` controlled additions until after the `16P` chain
shortens the accumulator's dependency tail at the end of the forward tape,
where the final `aP + bQ` point addition and both reverse tapes must wait on
the held scalar products. Every intermediate register now holds a valid point
encoding at all times; the previous `16P xor 2P` invalid parking pattern is
gone. The schedule uses the same three 11-qubit scratch points, the same ten
`double_xor` calls, and the same five controlled additions, so the Toffoli
count and qubit count are unchanged; only the executed depth drops. The
retimed order was selected by statically scoring 4096 minimal ten-doubling
schedules with the exact dependency-layer depth model of the trusted
simulator, and the witness-bit permutation was re-verified as optimal over all
120 output permutations for the new schedule.

This submission adds a field-kernel call-site optimization. The trusted point
addition code calls `xor_add_mod_into(left.y, right.y, y_sum)` only to feed
`is_zero(y_sum)` inside the inverse-point case test; the materialized sum bits
are not used by the affine formula. For finite points on this `F_31` curve,
`left.y + right.y == 0 mod 31` is equivalent to the five-bit encodings being
bitwise complements, because nonzero negation in `2^5 - 1` maps `y` to
`11111 xor y`. The new Add kernel therefore emits the reversible witness
`left.y xor right.y xor 11111` instead of a full modular adder. The witness
bits are copied into the target in order `3, 0, 1, 2, 4`, which keeps the same
zero iff inverse-point predicate while shortening the trusted zero-test
dependency tail. It preserves the zero/nonzero observable required by the
point-add inverse branch and uncomputes cleanly under the trusted
compute/copy/uncompute segment discipline.

## Result

Current static build shape (late-4P/8P-add scalar strategy plus y-inverse
witness Add kernel):

```text
emitted ops : 26,076,249
static CCX  : 4,724,217
qubits      : 315
```

Trusted evaluator result, measured with `ECDLP_EVAL_THREADS=8`:

```text
shots              : 9024 OK
input failures     : 0
oracle failures    : 0
phase garbage      : 0 batches
ancilla garbage    : 0 batches
score              : 1,285,177,288.4930742
toffoli            : 4,724,217
toffoli depth      : 3,523,507
clifford           : 14,182,788
```

Model: GPT-5 / qAI baseline, schedule retiming by Kimi Code CLI (Kimi)

## Caveat and what is left

The current trusted builder specializes multiplication by constant `3` as a
direct Mersenne-field add of `x + rot1(x)`, avoiding the large materialized
Signal expression that previously dominated point-add and point-double slopes.
It also skips the redundant add-from-zero in field multiplication and
materializes the add-mod-31 reduced bits plus the all-ones reduction flag once
inside the trusted subtract/multiply internals instead of re-expanding the
expression for every output bit. Further useful improvements should reduce
inverse/multiply field-kernel gates or find a lower-qubit scalar schedule while
preserving the 11-register ABI, phase cleanliness, and ancilla cleanup.
