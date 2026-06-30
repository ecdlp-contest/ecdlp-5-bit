## Submission Summary

- Approach:
- Field arithmetic file changed:
- Architecture or memory notes changed:
- Memory note added:
- Website submission ID:
- Env/config:
- Nonce/reroll:
- Model/provenance:

## Claimed Metrics

- Score:
- Score model:
- Toffoli:
- CCX:
- CCZ:
- Clifford:
- Qubits:
- Ops:
- Classical/phase/ancilla:

## Validation

Command:

```bash
ecdlp setup
cargo fmt --check
ecdlp run --note "<note>"
pwsh -NoProfile -ExecutionPolicy Bypass -File tools/package-submission.ps1 -NoteFile src/shor_oracle/memory/README.md -Model "<model>"
ecdlp validate
```

Required output:

```text
all 9024 shots OK
input failures          : 0
oracle failures         : 0
phase-garbage batches   : 0
ancilla-garbage batches : 0
```

## Co-Author Credit

Name and email for accepted merge commit:

```text
Co-authored-by: Name <email@example.com>
```

## Checklist

- [ ] I did not modify trusted harness files unless this PR is explicitly infrastructure work.
- [ ] I included a short memory note explaining the approach.
- [ ] I ran the trusted evaluator locally.
- [ ] The submitted circuit passes the 9024-shot Fiat-Shamir oracle.
- [ ] I packaged only `benchmark.json` `editablePaths` with the package helper.
- [ ] The public note includes model/provenance and is within the 10 KiB cap.
