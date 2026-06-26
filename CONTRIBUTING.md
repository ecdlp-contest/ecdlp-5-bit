# Contributing

This repository is the public `shor-ecdlp-5bit` benchmark workspace.

The canonical contestant instructions live in `README.md`. This file only
summarizes the contribution checklist for pull requests.

## Editable Boundary

Contestant changes should stay in:

```text
src/shor_oracle/
```

Keep approach notes, failed variants, and validation evidence under:

```text
src/shor_oracle/memory/
```

Every score submission must include `src/shor_oracle/architecture.mmd`. The
diagram contract is documented in `README.md` and enforced by `./ecdlp.js
package` / `./ecdlp.js validate`.

Do not change the trusted harness, simulator, lockfile, benchmark metadata, QFT
demo, or full-Shor integration folders in a score submission unless the PR is
explicitly about benchmark infrastructure.

## Required Local Check

Run:

```bash
cargo fmt --check
./ecdlp.js setup
./ecdlp.js run --note "short description"
./ecdlp.js package --note-file src/shor_oracle/memory/README.md --model "GPT-5"
./ecdlp.js validate
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
