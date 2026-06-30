# Shor Oracle Track

Folder:

```text
src/shor_oracle/
```

This is the scored Track 1 implementation. The editable implementation emits
the reversible oracle:

```text
|a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>
```

The trusted evaluator validates 9024 Fiat-Shamir oracle shots. It does not add
public `P+Q` / `2P` witnesses or hidden extra-modulus field-kernel probes. The
scored source must use reversible in-place `F_31` arithmetic rather than
enumerated point or field lookup tables. The score is
`qubits * sqrt(round(toffoli) * round(toffoli_depth))`, where `toffoli_depth`
is the average per-shot executed Toffoli depth before rounding and
is measured from CCX/CCZ dependency layers in the emitted op stream.

Submissions must include `src/shor_oracle/architecture.mmd`. See `README.md`
for the canonical public submission flow and exact diagram contract.
