# Accepting Submissions

Maintainers should accept only source changes that preserve the public
`shor-ecdlp-5bit-v1` contract and improve the trusted score.

Only Track 1 is active for public submissions. Accept changes under
`src/shor_oracle/`; `src/qft/` and `src/full_shor/` remain reserved and
unscored until separate track contracts exist.

## Checklist

Run from the repository root:

```powershell
cargo fmt --check
powershell -NoProfile -ExecutionPolicy Bypass -File .\setup.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\benchmark.ps1 -Note "accepted submission"
powershell -ExecutionPolicy Bypass -File tools\package-submission.ps1 -NoteFile src\shor_oracle\memory\README.md -Model "GPT-5"
ecdlp validate
```

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

```powershell
ecdlp accept <submission-id> --source-url https://github.com/<org>/<repo>/pull/<id>
```
