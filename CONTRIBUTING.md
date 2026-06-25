# Contributing

This repository is the public `shor-ecdlp-5bit` benchmark workspace.

## What To Edit

Contestant changes should stay in:

```text
src/shor_oracle/
```

Add notes about the approach, failed variants, and validation evidence under:

```text
src/shor_oracle/memory/
```

Each submission must include a Mermaid architecture diagram:

```text
src/shor_oracle/architecture.mmd
```

The diagram must be at most 1 MiB and contain these exact top-level anchors:

```text
Target oracle: aG + bQ
Algorithm
Optimization
```

The target anchor must branch to `Algorithm` and `Optimization`. Use the
algorithm branch for the oracle structure and the optimization branch for
search islands, structural knobs, score tradeoffs, and the chosen
implementation.

Do not change the trusted harness, simulator, lockfile, or benchmark metadata in
a score submission unless the PR is explicitly about benchmark infrastructure.

## Required Local Check

Run:

```bash
cargo fmt --check
ecdlp setup
ecdlp run --note "short description"
pwsh -NoProfile -ExecutionPolicy Bypass -File tools/package-submission.ps1 -NoteFile src/shor_oracle/memory/README.md -Model "GPT-5"
ecdlp validate
```

A score claim is only meaningful when the evaluator reports:

```text
all 9024 shots OK
input failures          : 0
oracle failures         : 0
phase-garbage batches   : 0
ancilla-garbage batches : 0
```

`ops.bin`, `score.json`, and `results.tsv` are generated and ignored. Do not
hand-edit benchmark output.

## Pull Request Requirements

Include:

- score
- toffoli
- ccx and ccz
- qubits
- emitted ops
- the exact validation command
- the exact package command
- confirmation that `src/shor_oracle/architecture.mmd` explains the submitted algorithm and optimization path
- model/provenance for the public submission note
- a short explanation of the approach
- public website submission ID, if you uploaded one
- the name and email to use for the accepted `Co-authored-by:` trailer

## Acceptance

Maintainers rerun the trusted evaluator. Accepted score improvements are
squash-merged into `main` with the submitter credited as a co-author. See
`docs/ACCEPTING_SUBMISSIONS.md` for the maintainer checklist.
