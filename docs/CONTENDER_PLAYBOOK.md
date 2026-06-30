# Contender Playbook

The baseline is a reversible arithmetic implementation of the 5-bit Shor oracle:

```text
|a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>
```

A good optimization keeps the 11-register ABI unchanged while reducing CCX
count, qubits, or both. The trusted `src/shor_oracle/mod.rs` composer freezes
the point and scalar-multiplication shape; submitted code changes should
optimize `src/shor_oracle/field_arithmetic.rs` using in-place reversible
`F_31` arithmetic kernels, not enumerated lookup tables. There are no hidden
`F_17` or `F_19` field-kernel validation shots.

## Loop

1. Edit `src/shor_oracle/field_arithmetic.rs`, `src/shor_oracle/architecture.mmd`, and notes under `src/shor_oracle/memory/`.
2. Run `cargo fmt --check`.
3. Run `./ecdlp.js run --note "short experiment label"` or `.\benchmark.ps1`.
4. Record score, Toffoli, qubits, ops, and the idea tested in
   `src/shor_oracle/memory/`.
5. Update `src/shor_oracle/architecture.mmd` with the submitted algorithm
   shape and optimization path.
6. Follow the package, validate, and submit flow in `README.md` only after a
   trusted ranked run.

## Architecture Diagram

Submissions must include `src/shor_oracle/architecture.mmd`. `README.md`
is the canonical source for the exact diagram contract; use this playbook only
for optimization workflow notes.

## Useful Directions

- Reuse scratch inside field kernels so fewer qubits remain live between
  compute, output-copy, and uncompute.
- Optimize the field kernels used by scalar multiplication and the final
  `aP + bQ` point addition.
- Specialize the `F_31` field kernels for `31 = 2^5 - 1` while keeping them
  algorithmic rather than table-enumerated.
- Push pebbling inside scalar multiplication instead of holding full expression
  trees until the segment boundary.
- Trade a small amount of scratch for fewer repeated equality checks.
- Preserve the input registers `a`, `b`, `P`, and `Q`; the trusted evaluator
  rejects mutations.

## Validation Boundary

Scanner-clean or shape-only evidence is not enough. A submission is meaningful
only when the trusted evaluator reports all 9024 Fiat-Shamir oracle shots OK,
zero phase garbage, and zero ancilla garbage.
