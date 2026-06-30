# Accepting Submissions

Maintainers should accept only source changes that preserve the public
`shor-ecdlp-5bit` contract and improve the trusted score.

Only Track 1 is active for public submissions. Accept code changes under
`src/shor_oracle/field_arithmetic.rs` and
`src/shor_oracle/scalar_strategy.rs` plus required updates to
`src/shor_oracle/architecture.mmd` and `src/shor_oracle/memory/`.
`src/qft/` and `src/full_shor/` remain reserved and unscored until separate
track contracts exist.

`README.md` is the canonical contestant-facing submission guide. This document
is the maintainer-side acceptance checklist for reruns and merge decisions.

## Checklist

Run from the repository root:

```bash
cargo fmt --check
./ecdlp.js setup
./ecdlp.js run --note "accepted submission"
./ecdlp.js package --note-file src/shor_oracle/memory/README.md --model "GPT-5"
./ecdlp.js validate
```

Confirm `./ecdlp.js package` and `./ecdlp.js validate` accept
`src/shor_oracle/architecture.mmd`; the exact diagram contract is documented in
`README.md`.

Confirm validation does not report `FIELD_ARITHMETIC_BOUNDARY` or
`SCALAR_STRATEGY_BOUNDARY`. The editable source must not import raw qubits, raw
circuit ops, raw point registers, the trusted builder, unsafe code, mutable
global state, external data, or process/environment state. The scalar strategy
must use only opaque scalar-bit and point handles. This is the mechanical guard
against P/Q subgroup-index tables, direct `aP+bQ` tables, and replacement
point-oracle tables.

Confirm `score.json` contains:

```text
status: ranked
score_model: balanced-qubit-toffoli-depth-v1
validation.shots: 9024
validation.gate: fiat_shamir_shor_ecdlp_5bit_arithmetic_strategy_oracle_v2
```

The trusted run must report:

```text
input failures          : 0
oracle failures         : 0
phase-garbage batches   : 0
ancilla-garbage batches : 0
```

Package metadata must include:

```text
benchmark: shor-ecdlp-5bit
editablePaths: ["src/shor_oracle/field_arithmetic.rs", "src/shor_oracle/scalar_strategy.rs", "src/shor_oracle/architecture.mmd", "src/shor_oracle/memory"]
artifactBytes: <ops.bin byte size>
artifactSha256: <ops.bin sha256>
```

Compare `artifactBytes` and `artifactSha256` against the maintainer rerun for
the same accepted source. They are not expected to match the original baseline
after a real optimization changes `ops.bin`.

Use `tools/prepare-acceptance.ps1` to generate a commit-message draft with the
score, metrics, validation gate, model attribution, and co-author trailer.

If the submitter uploaded a package record to <https://ecdlp.ai>, accept it only
after the trusted rerun and source merge decision:

```bash
# Run from the private contest maintainer CLI/API, not this public baseline helper.
ecdlp accept <submission-id> --source-url https://github.com/<org>/<repo>/pull/<id>
```
