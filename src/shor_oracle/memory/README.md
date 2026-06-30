# Baseline Notes

This baseline supports a reversible arithmetic circuit for the variable-base
5-bit Shor ECDLP oracle:

```text
|a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>
```

The submitted code boundary is `src/shor_oracle/field_arithmetic.rs` and
`src/shor_oracle/scalar_strategy.rs`; the required editable documentation is
`src/shor_oracle/architecture.mmd` plus this `src/shor_oracle/memory/`
directory. The trusted `src/shor_oracle/mod.rs` composes the oracle, trusted
`src/shor_oracle/scalar_api.rs` exposes only opaque scalar and point handles,
and trusted `src/shor_oracle/builder.rs` owns register allocation, segment
boundaries, primitive op emission, and the opaque field facade. Contenders
optimize reversible in-place `F_31` field kernels and scalar point-power
scheduling instead of replacing the point/scalar-multiplication layer with P/Q
subgroup-index tables, direct `aP+bQ` tables, or an enumerated point oracle.
Selector-driven `F_13` or `F_11` witness lanes are not part of the public ABI
and do not increase the qubit count. There are also no hidden `F_17` or `F_19`
field-kernel validation shots.

The implementation is intentionally arithmetic-first and point-lookup-free at
the contract level. It builds prime-field add, subtract, multiply, inverse,
compare, zero-test, and mux operations as reversible Boolean networks. The
trusted builder now expands field kernels through ripple add/subtract circuits,
cyclic-shift multiplication over the `2^5 - 1` modulus, and a Fermat
exponentiation chain for inverse instead of enumerating input assignments into
field truth tables. Each trusted segment computes into scratch, copies only
required point outputs or held intermediate points, uncomputes the scratch, and
then reuses those qubits.

The current measured artifact uses the editable scalar strategy for dynamic
point-power precompute: for each variable input point, it computes `2P`, `4P`,
`8P`, and `16P` once, uses those powers for controlled scalar adds, and clears
the chain in reverse. The strategy can choose store/recompute schedules through
`scalar_api`, but cannot inspect raw point registers, emit primitive gates, or
select table-derived point values.

Current static build shape for the table-free field-circuit baseline:

```text
emitted ops : 209,409,729
static CCX  : 143,435,925
qubits      : 319
```

Trusted evaluator result, measured with `ECDLP_EVAL_THREADS=8`:

```text
shots              : 9024 OK
input failures     : 0
oracle failures    : 0
phase garbage      : 0 batches
ancilla garbage    : 0 batches
score              : 41,289,076,650.6699
toffoli            : 143,435,925
toffoli depth      : 116,796,825
clifford           : 55,900,618
```

Further useful improvements should recover some of the added live scratch or
reduce field-kernel gates further, while preserving the 11-register ABI, phase
cleanliness, and ancilla cleanup.
