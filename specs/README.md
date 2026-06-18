# Specs

`SubmissionWorkflow.tla` models the public submission lifecycle used by this repository:

- contestants may edit only manifest `editablePaths`;
- submission packages require a public note and model attribution;
- accepted submissions must pass the trusted 9024-shot Fiat-Shamir oracle validator and improve the lower-is-better frontier;
- sync/reset restores only editable paths from promoted submissions while harness files follow the default branch.

`SubmissionWorkflow.cfg` is a small TLC model. It uses tiny byte and shot bounds so model checking stays finite; the implementation uses the production constants in `tools/package-submission.ps1` and `benchmark.json`.
