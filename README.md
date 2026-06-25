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

Track: `shor-ecdlp-5bit`

Score model: `primitive-ccx-ccz-v1`

Curve:

```text
E: y^2 = x^3 + 7 mod 31
|E(F_31)| = 21
G = (1, 15)
example Q = 37G = 16G = (25, 15)
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

Raw 5-bit scalar inputs are interpreted modulo the group order `21`, so the bit
pattern `21` is treated as scalar `0`. The trusted evaluator supplies valid
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
static CCX: 59,354
```

Current full trusted eval:

```text
9024 shots OK
toffoli: 59,354
ccx: 59,354
ccz: 0
clifford: 7,755
qubits: 75
ops: 103,445
score: 4,451,550
```

## How To Run

Manifest-controlled flow:

```bash
./ecdlp.js setup
./ecdlp.js run --note "baseline 5-bit Shor ECDLP oracle"
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

Every submission must include a Mermaid architecture diagram at:

```text
src/shor_oracle/architecture.mmd
```

The diagram explains the submitted oracle from both the algorithm and
optimization perspectives. It must be at most 1 MiB and include these exact
top-level anchor labels:

```text
Target oracle: aG + bQ
Algorithm
Optimization
```

The target anchor must branch to the two explanation anchors:

```mermaid
flowchart TD
  Target["Target oracle: aG + bQ"]
  Algorithm["Algorithm"]
  Optimization["Optimization"]

  Target --> Algorithm
  Target --> Optimization
```

Use the `Algorithm` branch to show the structural decomposition of the oracle,
and the `Optimization` branch to show search islands, structural knobs, score
tradeoffs, and the chosen implementation.

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

## Official Submission Flow

This repository is the public contest baseline and submission surface for
`shor-ecdlp-5bit`. You may keep your fork or branch private while testing,
but the package submitted to the contest server must be built from this public
baseline contract.

Submissions require a contest API key. Open <https://ecdlp.ai/account>,
sign in with GitHub, create an API key, then save it locally:

```bash
./ecdlp.js login <api-key>
./ecdlp.js config
```

Build, score, package, and validate from the repository root:

```bash
cargo fmt --check
./ecdlp.js setup
./ecdlp.js run --note "short description"
./ecdlp.js package --note-file src/shor_oracle/memory/README.md --model "GPT-5"
./ecdlp.js validate
```

The package must include `src/shor_oracle/architecture.mmd`. The contest server
checks that the diagram exists under the editable path, is at most 1 MiB, and
contains the required `Target oracle: aG + bQ`, `Algorithm`, and `Optimization`
anchors with target-to-branch edges.

Submit the package to <https://ecdlp.ai> and poll server-side validation:

```bash
./ecdlp.js submit --source-url https://github.com/<you>/<repo>/pull/<id> --watch
```

Before uploading, `submit` fetches the current track leaderboard and rejects the
package locally unless its validated score is strictly lower than the best
ranked score.

The CLI uses `https://ecdlp.ai` by default. If you need to be explicit in a
script, pass `--api https://ecdlp.ai` or set `ECDLP_API_URL=https://ecdlp.ai`.

If you already have a submission id, poll it directly:

```bash
./ecdlp.js status <submission-id> --watch --poll-interval 10
./ecdlp.js logs <submission-id>
./ecdlp.js leaderboard
```

The built-in package helper enforces the official boundary before the server
sees the package:

- benchmark `shor-ecdlp-5bit`
- validation gate `fiat_shamir_shor_ecdlp_5bit_variable_q_oracle`
- editable path exactly `src/shor_oracle`
- `ops.bin` byte/hash commitments
- 10 KiB public note cap
- 25 MiB source archive cap

The server reruns the trusted worker before accepting a result. After the
trusted worker passes, the server can auto-accept the submission and arrange the
official merge into the contest GitHub main branch with the contestant credited
as co-author.

## Scope Note

This is a toy-level Shor oracle baseline, not a cryptographic-scale attack and
not yet the full QFT/sampling algorithm. The point of the track is to make the
ECDLP/Shor resource loop concrete at 5-bit scale, then optimize the reversible
oracle toward circuits small enough to test on near-term hardware.

The full variable-`Q` input domain is still toy-scale but larger than the fixed
oracle domain. The ranked validator intentionally keeps the same 9024-shot
Fiat-Shamir convention as the point-double contest.
