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
`round(toffoli) * qubits`.
