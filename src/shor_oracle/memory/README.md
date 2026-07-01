# Baseline Notes

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
level. It builds prime-field add, subtract, multiply, inverse, compare,
zero-test, and mux operations as reversible Boolean networks. The trusted
builder expands field kernels through ripple add/subtract circuits,
cyclic-shift multiplication over the `2^5 - 1` modulus, and a Fermat
exponentiation chain for inverse instead of enumerating input assignments into
field truth tables. Each trusted segment computes into scratch, copies only
required point outputs or held intermediate points, uncomputes the scratch, and
then reuses those qubits.

This submission keeps the accepted 3-point pebbling strategy but changes which
point-power slot is recycled. The previous frontier held `w1=4P` and `w2=8P`,
freed the `w0=2P` slot, computed `16P` into `w0`, and then had to recompute
`2P` to clean `4P`. This version keeps `w0=2P`, frees the `w1=4P` slot,
computes `16P = 2·(8P)` into that slot, and then recreates `4P` from the still
available `2P` to clean `8P`. It uses the same three 11-qubit scratch points and
the same high-level API calls, but the cleanup chain is cheaper under the
trusted builder's point-double circuit because it avoids the base-point cleanup
path in the middle of the schedule.

Current static build shape (3-point `w1`-reuse scalar strategy):

```text
emitted ops : 26,097,795
static CCX  : 4,734,423
qubits      : 315
```

Trusted evaluator result, measured with `ECDLP_EVAL_THREADS=8`:

```text
shots              : 9024 OK
input failures     : 0
oracle failures    : 0
phase garbage      : 0 batches
ancilla garbage    : 0 batches
score              : 1,288,292,162.6386697
toffoli            : 4,734,423
toffoli depth      : 3,532,975
clifford           : 14,194,938
```

Model: GPT-5.5 / Hermes Agent

The current trusted builder specializes multiplication by constant `3` as a
direct Mersenne-field add of `x + rot1(x)`, avoiding the large materialized
Signal expression that previously dominated point-add and point-double slopes.
It also skips the redundant add-from-zero in field multiplication and
materializes the add-mod-31 reduced bits plus the all-ones reduction flag once
instead of re-expanding the expression for every output bit. Further useful
improvements should recover some of the added live scratch or reduce
inverse/multiply field-kernel gates further, while preserving the 11-register
ABI, phase cleanliness, and ancilla cleanup.
