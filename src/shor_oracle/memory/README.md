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

The implementation is intentionally arithmetic-first and point-lookup-free at
the contract level. It builds prime-field add, subtract, multiply, inverse,
compare, zero-test, and mux operations as reversible Boolean networks. The
trusted builder now expands field kernels through ripple add/subtract circuits,
cyclic-shift multiplication over the `2^5 - 1` modulus, and a Fermat
exponentiation chain for inverse instead of enumerating input assignments into
field truth tables. Each trusted segment computes into scratch, copies only
required point outputs or held intermediate points, uncomputes the scratch, and
then reuses those qubits.

The editable scalar strategy uses a 3-point pebbling schedule instead of the
naive 4-point chain. Two scratch points (`w1=4P`, `w2=8P`) are held throughout;
the third (`w0`) first stores `2P`, is uncomputed after bit 1, then reloaded
with `16P = 2·(w2)` for bit 4 before a clean reverse uncompute. This saves one
11-qubit scratch point (11 fewer logical qubits) at the cost of two extra
double operations, which is a net score improvement under the
`qubits × sqrt(toffoli × depth)` model.

Current static build shape (3-point pebble scalar strategy):

```text
emitted ops : 26,162,517
static CCX  : 4,791,417
qubits      : 315
```

Trusted evaluator result, measured with `ECDLP_EVAL_THREADS=8`:

```text
shots              : 9024 OK
input failures     : 0
oracle failures    : 0
phase garbage      : 0 batches
ancilla garbage    : 0 batches
score              : 1,304,571,408.6103384
toffoli            : 4,791,417
toffoli depth      : 3,579,733
clifford           : 14,202,022
```

Model: Claude Sonnet 4.6

The current trusted builder specializes multiplication by constant `3` as a
direct Mersenne-field add of `x + rot1(x)`, avoiding the large materialized
Signal expression that previously dominated point-add and point-double slopes.
It also skips the redundant add-from-zero in field multiplication and
materializes the add-mod-31 reduced bits plus the all-ones reduction flag once
instead of re-expanding the expression for every output bit. Further useful
improvements should recover some of the added live scratch or reduce
inverse/multiply field-kernel gates further, while preserving the 11-register
ABI, phase cleanliness, and ancilla cleanup.
