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

Submissions must include:

```text
src/shor_oracle/architecture.mmd
```

The Mermaid diagram must be at most 1 MiB and contain the exact top-level
anchors `Target oracle: aG + bQ`, `Algorithm`, and `Optimization`, with the
target anchor branching to both explanation anchors.
