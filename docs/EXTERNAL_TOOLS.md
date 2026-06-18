# External Tools

This baseline is self-contained. External Shor, synthesis, and routing tools can
be useful for design exploration, but trusted scoring is only the local
`build_circuit` plus `eval_circuit` flow in this repository.

If an external tool produces a candidate circuit, port the reusable idea into
`src/shor_oracle/` and rerun the trusted evaluator before making any score
claim.
