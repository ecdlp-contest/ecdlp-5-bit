# Baseline Notes

This baseline supports a reversible arithmetic circuit for the variable-base
5-bit Shor ECDLP oracle:

```text
|a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>
```

The submitted code boundary is `src/shor_oracle/field_arithmetic.rs`; the
required editable documentation is `src/shor_oracle/architecture.mmd` plus this
`src/shor_oracle/memory/` directory. The trusted `src/shor_oracle/mod.rs`
composes the oracle, so contenders optimize reversible in-place `F_31` field
kernels instead of replacing the point/scalar-multiplication layer with an
enumerated lookup. Selector-driven `F_13` or `F_11` witness lanes are not part
of the public ABI and do not increase the qubit count. There are also no hidden
`F_17` or `F_19` field-kernel validation shots.

The implementation is intentionally arithmetic-first and lookup-free at the
contract level. It builds prime-field add, subtract, multiply, inverse, compare,
zero-test, and mux operations as reversible Boolean networks. Each segment
computes into scratch, copies only required point outputs or held intermediate
points, uncomputes the scratch, and then reuses those qubits.

Current build shape after the bounded-register rewrite:

```text
emitted ops : 56,107,479
static CCX  : 38,941,319
qubits      : 205
```

Trusted evaluator result:

```text
shots              : 9024 OK
input failures     : 0
oracle failures    : 0
phase garbage      : 0 batches
ancilla garbage    : 0 batches
score              : 7,876,120,357.146168
toffoli            : 38,941,319
toffoli depth      : 37,905,856
clifford           : 10,337,118
```

Useful improvements should reduce the field-kernel gates without expanding
live scratch, while preserving the 11-register ABI, phase cleanliness, and
ancilla cleanup.
