# Contender Playbook

The baseline is a reversible arithmetic implementation of the 5-bit Shor oracle:

```text
|a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>
```

A good optimization keeps the 11-register ABI unchanged while reducing CCX
count, qubits, or both. The trusted `src/shor_oracle/mod.rs` composer freezes
the oracle and affine point formulas, trusted `src/shor_oracle/scalar_api.rs`
exposes opaque scalar-scheduling handles, and trusted
`src/shor_oracle/builder.rs` owns register allocation, segment boundaries, and
primitive op emission. Submitted code changes should optimize
`src/shor_oracle/field_arithmetic.rs` through the opaque field-kernel facade and
`src/shor_oracle/scalar_strategy.rs` through the opaque scalar API, not by
building P/Q subgroup-index tables, direct `aP+bQ` tables, or an enumerated
point oracle.

## Loop

1. Edit `src/shor_oracle/field_arithmetic.rs`, `src/shor_oracle/scalar_strategy.rs`, `src/shor_oracle/architecture.mmd`, and notes under `src/shor_oracle/memory/`.
2. Run `cargo fmt --check`.
3. Run `./ecdlp.js preflight` for cheap local and pull-request contract checks.
4. Run `./ecdlp.js run --note "short experiment label"` or `.\benchmark.ps1`
   only when validating a score or submission candidate.
5. Record score, Toffoli, qubits, ops, and the idea tested in
   `src/shor_oracle/memory/`.
6. Update `src/shor_oracle/architecture.mmd` with the submitted algorithm
   shape and optimization path.
7. Follow the package, validate, and submit flow in `README.md` only after a
   trusted ranked run.

## Repo-Local Builds

Keep build output and temporary files under `.workspace/` to avoid Windows
permission or application-control issues. This repo already configures Cargo to
use `.workspace/target`; if you invoke Cargo directly, make the target and temp
paths explicit:

```powershell
New-Item -ItemType Directory -Force .workspace\target, .workspace\tmp | Out-Null
$env:CARGO_TARGET_DIR = (Resolve-Path .workspace\target).Path
$env:TMP = (Resolve-Path .workspace\tmp).Path
$env:TEMP = (Resolve-Path .workspace\tmp).Path
cargo build --locked --release --bin build_circuit --bin eval_circuit
```

Use the generated binaries from `.workspace\target\release\`.

## Architecture Diagram

Submissions must include `src/shor_oracle/architecture.mmd`. `README.md`
is the canonical source for the exact diagram contract; use this playbook only
for optimization workflow notes.

## Useful Directions

- Use the opaque field emitter to reduce gates in the field kernels called by
  scalar multiplication and the final `aP + bQ` point addition.
- Use the opaque scalar API to trade point-power storage, recomputation, and
  cleanup without accessing raw point registers or raw gates.
- Specialize the `F_31` field kernels for `31 = 2^5 - 1` while keeping them
  scoped to their field operands rather than keyed by public point registers.
- Trade a small amount of scratch for fewer repeated equality checks.
- Preserve the input registers `a`, `b`, `P`, and `Q`; the trusted evaluator
  rejects mutations.

## Validation Boundary

Scanner-clean, preflight-clean, or shape-only evidence is not enough for a
submission. A submission is meaningful only when the trusted evaluator reports
all 9024 Fiat-Shamir oracle shots OK, zero phase garbage, and zero ancilla
garbage.
