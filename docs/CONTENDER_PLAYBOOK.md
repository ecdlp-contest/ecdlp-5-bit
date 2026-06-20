# Contender Playbook

The baseline is a direct reversible lookup for:

```text
|a>|b>|Q>|0> -> |a>|b>|Q>|aG + bQ>
```

A good optimization keeps the ABI unchanged while reducing CCX count, qubits, or
both.

## Loop

1. Edit only `src/shor_oracle/`.
2. Run `cargo fmt --check`.
3. Run `./ecdlp.js run --note "short experiment label"` or `.\benchmark.ps1`.
4. Record score, Toffoli, qubits, ops, and the idea tested in
   `src/shor_oracle/memory/`.
5. Package only after a trusted ranked run.

## Useful Directions

- Replace the table baseline with arithmetic for scalar multiplication by `G`
  and by the input point `Q`.
- Use the special modulus `31 = 2^5 - 1` to fold carries cheaply.
- Trade a small amount of scratch for fewer repeated equality checks.
- Preserve the input register `b`; the trusted evaluator rejects mutations.

## Validation Boundary

Scanner-clean or shape-only evidence is not enough. A submission is meaningful
only when the trusted evaluator reports all 9024 Fiat-Shamir oracle shots OK, zero phase
garbage, and zero ancilla garbage.
