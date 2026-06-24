# Accepting Submissions

Maintainers should accept only source changes that preserve the public
`shor-ecdlp-5bit-v1` contract and improve the trusted score.

Only Track 1 is active for public submissions. Accept changes under
`src/shor_oracle/`; `src/qft/` and `src/full_shor/` remain reserved and
unscored until separate track contracts exist.

## Checklist

Run from the repository root:

```bash
cargo fmt --check
./ecdlp.js setup
./ecdlp.js run --note "accepted submission"
./ecdlp.js package --note-file src/shor_oracle/memory/README.md --model "GPT-5"
./ecdlp.js validate
```

Confirm the source tree includes `src/shor_oracle/architecture.mmd`. The diagram
must be at most 1 MiB and contain the required top-level anchors
`Target oracle: aG + bQ`, `Algorithm`, and `Optimization`, with the target
anchor branching to both explanation anchors.

Confirm `score.json` contains:

```text
status: ranked
score_model: primitive-ccx-ccz-v1
validation.shots: 9024
validation.gate: fiat_shamir_shor_ecdlp_5bit_variable_q_oracle
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
benchmark: shor-ecdlp-5bit-v1
editablePaths: ["src/shor_oracle"]
artifactBytes: <ops.bin byte size>
artifactSha256: <ops.bin sha256>
```

Compare `artifactBytes` and `artifactSha256` against the maintainer rerun for
the same accepted source. They are not expected to match the original baseline
after a real optimization changes `ops.bin`.

Use `tools/prepare-acceptance.ps1` to generate a commit-message draft with the
score, metrics, validation gate, model attribution, and co-author trailer.

If the submitter uploaded a package record to the contest website, accept it
only after the trusted rerun and source merge decision:

```bash
# Run from the private contest maintainer CLI/API, not this public baseline helper.
ecdlp accept <submission-id> --source-url https://github.com/<org>/<repo>/pull/<id>
```
