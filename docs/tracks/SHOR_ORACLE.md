# Shor Oracle Track

Scored oracle folder:

```text
src/shor_oracle/
```

This is the scored Track 1 implementation. The trusted oracle composer fixes
the reversible oracle shape:

```text
|a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>
```

The submitted code boundary is narrowed to
`src/shor_oracle/field_arithmetic.rs` and
`src/shor_oracle/scalar_strategy.rs`; submitted documentation remains
`src/shor_oracle/architecture.mmd` and `src/shor_oracle/memory/`. The trusted
builder owns register allocation, segment boundaries, and primitive op emission,
and passes only opaque per-field operands, scalar-bit handles, and point handles
to the editable source.
The trusted evaluator validates 9024 Fiat-Shamir oracle shots. It does not add
public `P+Q` / `2P` witnesses or hidden extra-modulus field-kernel probes. The
scored source must use reversible in-place `F_31` arithmetic and arithmetic
point-power scheduling rather than P/Q subgroup-index tables, direct `aP+bQ`
tables, or an enumerated point oracle. The
score is
`qubits * sqrt(round(toffoli) * round(toffoli_depth))`, where `toffoli_depth`
is the average per-shot executed Toffoli depth before rounding and
is measured from CCX/CCZ dependency layers in the emitted op stream.

Submissions must include `src/shor_oracle/architecture.mmd`. See `README.md`
for the canonical public submission flow and exact diagram contract.
