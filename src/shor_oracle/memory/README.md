# Baseline Notes

This baseline supports a reversible arithmetic circuit for the variable-base
5-bit Shor ECDLP oracle:

```text
|a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>
```

The submitted code boundary is `src/shor_oracle/field_arithmetic.rs`; the
required editable documentation is `src/shor_oracle/architecture.mmd` plus this
`src/shor_oracle/memory/` directory. The trusted `src/shor_oracle/mod.rs`
composes the oracle, and trusted `src/shor_oracle/builder.rs` owns register
allocation, segment boundaries, primitive op emission, and the opaque field
facade. Contenders optimize reversible in-place `F_31` field kernels instead of
replacing the point/scalar-multiplication layer with P/Q subgroup-index tables,
direct `aP+bQ` tables, or an enumerated point oracle. Selector-driven `F_13` or
`F_11` witness lanes are not part of the public ABI and do not increase the
qubit count. There are also no hidden `F_17` or `F_19` field-kernel validation
shots.

The implementation is intentionally arithmetic-first and point-lookup-free at
the contract level. It builds prime-field add, subtract, multiply, inverse,
compare, zero-test, and mux operations as reversible Boolean networks. The
trusted builder now expands field kernels through ripple add/subtract circuits,
cyclic-shift multiplication over the `2^5 - 1` modulus, and a Fermat
exponentiation chain for inverse instead of enumerating input assignments into
field truth tables. Each trusted segment computes into scratch, copies only
required point outputs or held intermediate points, uncomputes the scratch, and
then reuses those qubits.

Current static build shape for the table-free field-circuit baseline:

```text
emitted ops : 463,891,365
static CCX  : 329,681,671
qubits      : 291
```

Trusted evaluator result, measured with `ECDLP_EVAL_THREADS=8`:

```text
shots              : 9024 OK
input failures     : 0
oracle failures    : 0
phase garbage      : 0 batches
ancilla garbage    : 0 batches
score              : 86,618,639,258.06989
toffoli            : 329,681,671
toffoli depth      : 268,745,953
clifford           : 120,800,428
```

Useful improvements should reduce the field-kernel gates without expanding
live scratch, while preserving the 11-register ABI, phase cleanliness, and
ancilla cleanup.
