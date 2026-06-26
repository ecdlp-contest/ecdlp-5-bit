# Shor Oracle Track

Folder:

```text
src/shor_oracle/
```

This is the scored Track 1 implementation. It emits the reversible oracle:

```text
|a>|b>|Q>|0> -> |a>|b>|Q>|aG + bQ>
```

The trusted evaluator validates 9024 Fiat-Shamir shots and scores
`qubits * sqrt(round(toffoli) * round(toffoli_depth))`, where `toffoli_depth`
is the average per-shot executed Toffoli depth before rounding and
is measured from CCX/CCZ dependency layers in the emitted op stream.

Submissions must include `src/shor_oracle/architecture.mmd`. See `README.md`
for the canonical public submission flow and exact diagram contract.
