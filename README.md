# 5-bit Shor ECDLP Baseline

Goal: build the cheapest reversible oracle circuit for a toy end-to-end Shor
ECDLP demonstration, scored by `score = toffoli * qubits`.

This repository follows the ECDSA Fail baseline convention:

- contestant code lives under `src/shor_oracle/`;
- `build_circuit` is the untrusted build stage and emits `ops.bin`;
- `eval_circuit` is the trusted stage and never imports contestant code;
- the trusted evaluator validates 9024 Fiat-Shamir oracle shots;
- `score.json` and `results.tsv` record primitive CCX/CCZ metrics.

## Contract

Track: `shor-ecdlp-5bit-v1`

Score model: `primitive-ccx-ccz-v1`

Curve:

```text
E: y^2 = x^3 + 11x + 7 mod 31
|E(F_31)| = 31
G = (0, 10)
example Q = 37G = 6G = (28, 3)
```

Circuit ABI:

```text
register 0: scalar a              (5 qubits, preserved)
register 1: scalar b              (5 qubits, preserved)
register 2: input Q.x             (5 qubits, preserved)
register 3: input Q.y             (5 qubits, preserved)
register 4: input Q infinity flag (1 qubit, preserved)
register 5: output R.x            (5 qubits, initially zero)
register 6: output R.y            (5 qubits, initially zero)
register 7: output R infinity flag (1 qubit, initially zero)
```

The oracle must compute:

```text
|a>|b>|Q>|0> -> |a>|b>|Q>|aG + bQ>
```

Raw 5-bit scalar inputs are interpreted modulo the group order `31`, so the bit
pattern `31` is treated as scalar `0`. The trusted evaluator supplies valid
group points `Q = kG` after the circuit is built.

## Baseline

The baseline in `src/shor_oracle/mod.rs` is intentionally direct: it emits a
reversible table baseline that computes `A = aG`, `B = bQ`, then `R = A+B`
before uncomputing scratch. This gives the contest a legitimate variable-`Q`
Shor ECDLP oracle component before replacing the tables with prime field
arithmetic and adding QFT/sampling machinery.

Current expected static shape:

```text
input/output qubits: 32
lookup scratch: 43
logical qubits: 75
static CCX: 100,394
```

Current full trusted eval:

```text
9024 shots OK
toffoli: 100,394
ccx: 100,394
ccz: 0
clifford: 13,146
qubits: 75
ops: 174,472
score: 7,529,550
```

## How To Run

Manifest-controlled flow:

```bash
ecdlp setup
ecdlp run --note "baseline 5-bit Shor ECDLP oracle"
```

or directly:

```bash
./setup.sh
./benchmark.sh --note "baseline 5-bit Shor ECDLP oracle"
```

On Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\setup.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\benchmark.ps1 -Note "baseline 5-bit Shor ECDLP oracle"
```

The evaluator writes:

- `ops.bin`
- `score.json`
- `results.tsv`

## What You Can Edit

Contestant changes should stay in:

```text
src/shor_oracle/
```

Do not change the trusted harness when comparing submissions:

- `src/bin/build_circuit.rs`
- `src/bin/eval_circuit.rs`
- `src/main.rs`
- `src/circuit.rs`
- `src/sim.rs`
- `Cargo.toml`

## Implementation Folders

```text
src/shor_oracle/  scored oracle implementation
src/qft/          unscored QFT and sampling support
src/full_shor/    future full-Shor integration layer
```

Only `src/shor_oracle/` is part of the current submission boundary.

## Submission Flow

Before opening a PR, run:

```powershell
cargo fmt --check
ecdlp setup
ecdlp run --note "short description"
pwsh -NoProfile -ExecutionPolicy Bypass -File tools/package-submission.ps1 -NoteFile src/shor_oracle/memory/README.md -Model "GPT-5"
ecdlp validate
```

The package helper enforces:

- benchmark `shor-ecdlp-5bit-v1`
- validation gate `fiat_shamir_shor_ecdlp_5bit_variable_q_oracle`
- editable path `src/shor_oracle`
- 10 KiB public note cap
- 25 MiB source archive cap

## Scope Note

This is a toy-level Shor oracle baseline, not a cryptographic-scale attack and
not yet the full QFT/sampling algorithm. The point of the track is to make the
ECDLP/Shor resource loop concrete at 5-bit scale, then optimize the reversible
oracle toward circuits small enough to test on near-term hardware.

The full variable-`Q` input domain is still toy-scale but larger than the fixed
oracle domain. The ranked validator intentionally keeps the same 9024-shot
Fiat-Shamir convention as the point-double contest.
