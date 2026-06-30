# Contender Playbook

The baseline is a direct reversible lookup for the 5-bit Shor oracle plus a
separate `F_31`, `F_13`, and `F_11` field-witness ABI:

```text
|a>|b>|P>|Q>|field inputs>|0> -> |a>|b>|P>|Q>|field inputs>|aP + bQ>|P+Q>|2P>|field witnesses>
```

A good optimization keeps the ABI unchanged while reducing CCX count, qubits, or
both.

## Loop

1. Edit only `src/shor_oracle/`.
2. Run `cargo fmt --check`.
3. Run `./ecdlp.js run --note "short experiment label"` or `.\benchmark.ps1`.
4. Record score, Toffoli, qubits, ops, and the idea tested in
   `src/shor_oracle/memory/`.
5. Update `src/shor_oracle/architecture.mmd` with the submitted algorithm shape
   and optimization path.
6. Follow the package, validate, and submit flow in `README.md` only after a
   trusted ranked run.

## Architecture Diagram

Submissions must include `src/shor_oracle/architecture.mmd`. `README.md` is the
canonical source for the exact diagram contract; use this playbook only for
optimization workflow notes.

## Useful Directions

- Replace the table baseline with arithmetic for variable-base scalar
  multiplication by `P` and `Q`.
- Share point-addition and point-doubling arithmetic between the oracle output
  and the explicit `P+Q` / `2P` check outputs.
- Replace the selector-driven `F_31`, `F_13`, and `F_11` witness tables with
  reusable field add, subtract, multiply, inverse, and lambda-check circuits.
- Use the special field modulus `31 = 2^5 - 1` to fold carries cheaply.
- Trade a small amount of scratch for fewer repeated equality checks.
- Preserve the input registers `b`, `x1`, `y1`, `x2`, and `y2`; the trusted
  evaluator rejects mutations.

## Validation Boundary

Scanner-clean or shape-only evidence is not enough. A submission is meaningful
only when the trusted evaluator reports all 9024 Fiat-Shamir oracle and
point-operation shots OK, all selected `F_31`, `F_13`, and `F_11` field-witness shots OK, zero phase
garbage, and zero ancilla garbage.
