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

This submission keeps the accepted 3-point cleanup-pebble strategy, late
controlled-add placement, and table-free field arithmetic. The baseline computes
`2P` and `4P`, clears the `2P` slot, reuses it as `8P`, computes `16P` into the
third slot, and then turns that third slot into a `2P` cleanup pebble by toggling
`2P` and removing `16P`. The temporary `16P xor 2P` bit pattern is never used as
a point source or controlled addend; it is only an output slot on the way back to
a valid `2P` cleanup value.

The new optimization borrows the passenger-relocation/live-range idea from the
ecdsa.fail optimization primer: the third scratch point `w2` is allocated only
when `16P` is first needed instead of at the top of the scalar schedule. This
does not change the number of point powers, doublings, controlled additions,
scratch points, qubits, or Toffoli gates. It changes which recycled qubit ids the
trusted builder assigns to the cleanup pebble, shaving a small dependency-depth
tail from the emitted circuit while preserving the same arithmetic schedule.

The new schedule delays the low controlled additions until the relevant point
powers are about to be cleaned: add `16P`, park the cleanup pebble, add `8P`,
clean `8P`, add `4P`, clean `4P`, add `2P`, clean `2P`, and add the base point
last. This uses the same three 11-qubit scratch points, the same ten
`double_xor` calls, the same five controlled additions, and the same static
Toffoli count as the latest accepted baseline, but it shortens the trusted
Toffoli dependency depth by avoiding early accumulator dependencies on point
registers that still need to drive later doubles.

Current static build shape (late-add cleanup-pebble scalar strategy with delayed
cleanup-pebble allocation):

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
score              : 1,287,633,075.1625946
toffoli            : 4,734,423
toffoli depth      : 3,529,361
clifford           : 14,194,938
```

Model: GPT-5 / qAI

The current trusted builder specializes multiplication by constant `3` as a
direct Mersenne-field add of `x + rot1(x)`, avoiding the large materialized
Signal expression that previously dominated point-add and point-double slopes.
It also skips the redundant add-from-zero in field multiplication and
materializes the add-mod-31 reduced bits plus the all-ones reduction flag once
instead of re-expanding the expression for every output bit. Further useful
improvements should reduce inverse/multiply field-kernel gates or find a lower
qubit scalar schedule while preserving the 11-register ABI, phase cleanliness,
and ancilla cleanup.
