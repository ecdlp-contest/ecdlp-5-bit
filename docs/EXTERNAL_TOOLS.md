# External Tools

This baseline is self-contained. External Shor, synthesis, and routing tools can
be useful for design exploration, but trusted scoring is only the local
`build_circuit` plus `eval_circuit` flow in this repository.

If an external tool produces a candidate field-kernel circuit or scalar
point-power schedule, port the reusable idea into
`src/shor_oracle/field_arithmetic.rs` or `src/shor_oracle/scalar_strategy.rs`
and rerun the trusted evaluator before making any score claim.
