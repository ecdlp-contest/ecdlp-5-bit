# Accepting Submissions

Maintainers should accept only source changes that preserve the public
`shor-ecdlp-5bit` contract and improve the trusted score.

Only Track 1 is active for public submissions. Accept changes under
`src/shor_oracle/`; `src/qft/` and `src/full_shor/` remain reserved and
unscored until separate track contracts exist.

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

Confirm `score.json` contains:

```text
status: ranked
score_model: balanced-qubit-toffoli-depth-v1
validation.shots: 9024
validation.gate: fiat_shamir_shor_ecdlp_5bit_variable_base_point_ops_oracle_field_arithmetic_v4
```

The trusted run must report:

```text
input failures          : 0
oracle failures         : 0
point-add failures      : 0
point-double failures   : 0
field failures          : 0
phase-garbage batches   : 0
ancilla-garbage batches : 0
```

Package metadata must include:

```text
benchmark: shor-ecdlp-5bit
editablePaths: ["src/shor_oracle"]
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
