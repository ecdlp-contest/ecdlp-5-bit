# Baseline Notes

This baseline supports a reversible arithmetic circuit for the variable-base
5-bit Shor ECDLP oracle:

```text
|a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>
```

The submitted boundary is `src/shor_oracle`. `mod.rs` composes the oracle, while
`field_arithmetic.rs` supplies the in-place `F_31` field kernels and
scratch-management primitives. Selector-driven `F_13` or `F_11` witness lanes
are not part of the public ABI and do not increase the qubit count. There are
also no hidden `F_17` or `F_19` field-kernel validation shots.

The implementation is intentionally arithmetic-first and lookup-free. It builds
prime-field add, subtract, multiply, inverse, compare, zero-test, and mux
operations as reversible Boolean networks. Each segment computes into scratch,
copies only required point outputs or held intermediate points, uncomputes the
scratch, and then reuses those qubits.

Useful improvements should push scratch reuse inside scalar multiplication and
factor reusable in-place `F_31` field kernels while preserving the 11-register
ABI, phase cleanliness, and ancilla cleanup.
