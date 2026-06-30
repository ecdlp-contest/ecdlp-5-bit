# 5-bit Shor ECDLP Oracle Baseline

Goal: build the cheapest reversible oracle circuit for a 5-bit toy Shor ECDLP
oracle, scored by `score = qubits * sqrt(toffoli * toffoli_depth)`, where
`toffoli` is the rounded average executed Toffoli count and `toffoli_depth` is
the rounded average per-shot executed Toffoli depth. This is an important step aiming for running the full Shor's ECDLP algorithm on quantum hardware.

## Why This Matters

This toy 5-bit track makes the expensive reversible oracle inside Shor's ECDLP
loop concrete enough to build, test, and optimize end to end. A lower oracle
score means fewer non-Clifford resources and fewer live qubits in the part of
the circuit that dominates repeated group arithmetic.

This repository follows the [ECDSA Fail](https://ecdsa.fail) baseline convention:

- contestant code lives under `src/shor_oracle/`;
- `build_circuit` is the untrusted build stage and emits `ops.bin`;
- `eval_circuit` is the trusted stage and never imports contestant code;
- the trusted evaluator validates 9024 Fiat-Shamir oracle shots;
- `score.json` and `results.tsv` record primitive CCX/CCZ and Toffoli-depth metrics.

## AI Agent Quick Start

If you are using an AI coding agent, paste this prompt into the agent:

```text
Install the 5-bit Shor ECDLP contest CLI and open the contest repo:

curl -fsSL https://ecdlp.ai/install.sh | sh
cd "$(ecdlp repo)"

Use the CLI help to learn the workflow before acting:

ecdlp --help
ecdlp setup --help
ecdlp run --help
ecdlp package --help
ecdlp validate --help
ecdlp submit --help

Then read README.md, benchmark.json, ./ecdlp.js, src/shor_oracle/mod.rs,
src/shor_oracle/field_arithmetic.rs, src/shor_oracle/architecture.mmd, and
src/shor_oracle/memory/README.md.

Goal: improve the scored oracle under src/shor_oracle/ only. Do not edit the
trusted harness, Cargo.toml, Cargo.lock, rust-toolchain, score.json, ops.bin, or
results.tsv by hand.

Use repo-local build and scratch paths under .workspace/ to avoid permission
issues. This repo already routes Cargo builds to .workspace/target. If you need
extra caches, generated probes, temporary files, or tool downloads, put them
under .workspace/ and do not rely on system/global writable directories.

Local work does not require an API key. The user only needs to sign in with
GitHub and create an API key when they are ready to submit to ecdlp.ai.

Use this local loop:
1. Run ecdlp setup if the repo is not already prepared.
2. Modify src/shor_oracle/ and update src/shor_oracle/architecture.mmd plus
   src/shor_oracle/memory/README.md with the approach and result.
3. Run cargo fmt --check and ecdlp run --note "short description".
4. Package with ecdlp package --note-file src/shor_oracle/memory/README.md --model "<model-name>".
5. Run ecdlp validate before proposing submission.

A valid submission must beat the current best score, preserve the documented
oracle ABI, pass all 9024 trusted shots, include the Mermaid architecture
diagram, and explain the algorithm and optimization choices in the note.

When ready to submit, ask the user to open https://ecdlp.ai/account, sign in
with GitHub, create an API key, and run:

ecdlp login <api-key>
ecdlp submit --watch
```

## Benchmark

The harness:

1. builds an op stream by running the untrusted `src/shor_oracle`
   implementation;
2. validates 9024 Fiat-Shamir shots against
   `|a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>`;
3. checks oracle correctness, in-place `F_31` field-arithmetic composition,
   input preservation, phase cleanliness, and ancilla cleanup;
4. scores the run as logical qubits times the square root of rounded average
   executed Toffoli count times rounded average per-shot executed Toffoli depth.

Track: `shor-ecdlp-5bit`

Score model: `balanced-qubit-toffoli-depth-v1`

Curve:

```text
E: y^2 = x^3 + 7 mod 31
|E(F_31)| = 21
sampler base point = (1, 15)
example Q = 37B = 16B = (25, 15)
```

Circuit ABI:

```text
register 0: scalar a              (5 qubits, preserved)
register 1: scalar b              (5 qubits, preserved)
register 2: input P.x             (5 qubits, preserved)
register 3: input P.y             (5 qubits, preserved)
register 4: input P infinity flag (1 qubit, preserved)
register 5: input Q.x             (5 qubits, preserved)
register 6: input Q.y             (5 qubits, preserved)
register 7: input Q infinity flag (1 qubit, preserved)
register 8: output R.x            (5 qubits, initially zero)
register 9: output R.y            (5 qubits, initially zero)
register 10: output R infinity flag (1 qubit, initially zero)
```

The oracle must compute:

```text
|a>|b>|P>|Q>|0> -> |a>|b>|P>|Q>|aP + bQ>
```

Raw 5-bit scalar inputs are interpreted modulo the group order `21`, so the bit
pattern `21` is treated as scalar `0`. The trusted evaluator supplies valid
group points `P = sB` and `Q = tB` after the circuit is built, where `B` is the
sampler base point above.

The scored ABI intentionally has no hidden field-test registers. The only field
in the scored circuit is the curve field `F_31`; the trusted evaluator checks
only the oracle output and does not run hidden extra-modulus probes such as
`field_add_kernel(F_17)` or `field_mul_kernel(F_19)`. Submissions are expected
to implement reversible arithmetic over the five-bit `F_31` field elements.
Enumerated point or field lookup tables are outside the contest contract even
if they happen to pass the black-box shots.

### What Valid Means

A run is rejected if any of the following fails:

- Oracle correctness: all 9024 Fiat-Shamir shots must produce the expected
  `aP + bQ` output point.
- In-place field arithmetic composition: the submitted source must build the
  oracle from reversible `F_31` arithmetic rather than enumerated lookup tables.
- Input preservation: the `a`, `b`, `P`, and `Q` input registers must remain
  unchanged.
- Phase cleanliness: no leftover global phase may remain across the simulated
  shot batch.
- Ancilla cleanup: every non-register qubit must end in zero after the oracle
  runs.

## Baseline

The baseline is intentionally arithmetic-first. `src/shor_oracle/mod.rs` fixes
the oracle shape: affine point-add, point-double, and double-and-add scalar
multiplication. `src/shor_oracle/field_arithmetic.rs` provides the reversible
`F_31` add, subtract, multiply, inverse, compare, zero-test, mux, scratch
allocation, and compute/copy/uncompute primitives. The network computes
`A = aP`, `B = bQ`, and `R = A+B` into scratch, copies the oracle output into
the ABI output registers, and then runs the scratch network backward. No
enumerated point or field lookup tables are used.

Current expected static shape:

| Metric | Value |
| --- | ---: |
| Input/output qubits | 43 |
| Arithmetic scratch | 75,188 |
| Logical qubits | 75,231 |
| Static CCX | 290,342 |

Current full trusted eval:

| Metric | Value |
| --- | ---: |
| Shots | 9024 OK |
| Scored Toffoli count | 290,342 |
| CCX | 290,342 |
| CCZ | 0 |
| Avg. executed Toffoli depth | 25,105 |
| Clifford | 621,539 |
| Qubits | 75,231 |
| Ops | 956,311 |
| Score | 6,422,910,636.018275 |

`Static CCX` is the emitted gate count in `ops.bin`. The scored Toffoli count is
the rounded average executed `CCX + CCZ` count across the 9024 Fiat-Shamir shots,
matching the Google resource-estimate convention. This arithmetic baseline is
correct and lookup-free. It uses coarse Bennett pebbling: compute a segment,
copy its ABI or held-scratch outputs, uncompute the segment, and reuse the freed
scratch qubits. Field arithmetic is in-place at the oracle contract level:
computed field values are copied only into required point-output registers,
then arithmetic scratch is uncomputed. A competitive submission should push
scratch reuse inside scalar multiplication, which is now the peak live-qubit
segment.

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
Target oracle: aP + bQ using in-place F_31 field arithmetic
Algorithm
Optimization
```

The target anchor must branch to the two explanation anchors:

```mermaid
flowchart TD
  Target["Target oracle: aP + bQ using in-place F_31 field arithmetic"]
  Algorithm["Algorithm"]
  Optimization["Optimization"]

  Target --> Algorithm
  Target --> Optimization
```

Use the `Algorithm` branch to show the structural decomposition of the oracle,
and the `Optimization` branch to show search islands, structural knobs, score
tradeoffs, and the chosen implementation.

As you iterate, keep Markdown notes under `src/shor_oracle/memory/` capturing
approaches that worked, approaches that failed, and the reasoning behind
important choices. Treat existing notes as leads: verify claims and rerun the
benchmark before relying on them.

Do not change the trusted harness when comparing submissions:

- `src/bin/build_circuit.rs`
- `src/bin/eval_circuit.rs`
- `src/main.rs`
- `src/circuit.rs`
- `src/sim.rs`
- `Cargo.toml`

Implementation folders:

```text
src/shor_oracle/  scored oracle and reversible field-arithmetic implementation
src/qft/          unscored QFT and sampling support
src/full_shor/    future full-Shor integration layer
```

## Local Workflow

Use `ecdlp` after installing from `https://ecdlp.ai/install.sh`. If you cloned
the repo manually, run `./ecdlp.js` from the repo root instead.

```bash
ecdlp setup
ecdlp run --note "short description"
```

The evaluator writes `ops.bin`, `score.json`, and `results.tsv`. These are
generated benchmark artifacts; do not hand-edit them.

For a submission candidate:

```bash
cargo fmt --check
ecdlp package --note-file src/shor_oracle/memory/README.md --model "GPT-5"
ecdlp validate
```

The package helper enforces the official boundary before the server sees the
package:

- benchmark `shor-ecdlp-5bit`
- validation gate `fiat_shamir_shor_ecdlp_5bit_in_place_field_arithmetic_oracle_v1`
- editable path exactly `src/shor_oracle`
- `src/shor_oracle/architecture.mmd` commitment
- `ops.bin` byte/hash commitment
- 10 KiB public note cap
- 25 MiB source archive cap

Direct script entrypoints still work:

```bash
./setup.sh
./benchmark.sh --note "short description"
```

On Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\setup.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File .\benchmark.ps1 -Note "short description"
```

## Submit

Submissions require a contest API key. Open <https://ecdlp.ai/account>, sign in
with GitHub, create an API key, then save it locally:

```bash
ecdlp login <api-key>
ecdlp config
```

Submit the validated package and poll server-side validation:

```bash
ecdlp submit --watch
```

Before uploading, `submit` fetches the current track leaderboard and rejects the
package locally unless its validated score is strictly lower than the best
ranked score. `--source-url` is optional; use it when you have a public PR you
want reviewers or merge automation to see.

If you already have a submission id, poll it directly:

```bash
ecdlp status <submission-id> --watch --poll-interval 10
ecdlp logs <submission-id>
ecdlp leaderboard
```

The server reruns the trusted worker before accepting a result. After the
trusted worker passes, the server can auto-accept the submission and arrange the
official merge into the contest GitHub main branch with the contestant credited
as co-author.

## Documentation Map

- `README.md`: canonical benchmark contract and public submission flow.
- `CONTRIBUTING.md`: short pull-request checklist for score submissions.
- `docs/CONTENDER_PLAYBOOK.md`: optimization strategy and implementation ideas.
- `docs/ACCEPTING_SUBMISSIONS.md`: maintainer rerun and acceptance checklist.
- `docs/tracks/`: compact status notes for scored and reserved track folders.

## Scope Note

This is a toy-level Shor oracle baseline, not a cryptographic-scale attack and
not yet the full QFT/sampling algorithm. The point of the track is to make the
ECDLP/Shor resource loop concrete at 5-bit scale, then optimize the reversible
oracle toward circuits small enough to test on near-term hardware.

The full variable-`Q` input domain is still toy-scale but larger than the fixed
oracle domain. The ranked validator intentionally keeps the same 9024-shot
Fiat-Shamir convention as the point-double contest.

## Credits
This 5-bit Shor's ECDLP oracle contest was inspired by [https://ecdsa.fail](https://ecdsa.fail) and Google's paper
["Securing Elliptic Curve Cryptocurrencies against Quantum Vulnerabilities:
Resource Estimates and Mitigations"](https://arxiv.org/pdf/2603.28846). We thank the ecdsa-fail community for pioneering this effort.

5-bit ECDLP visualization was from [@jackylee0424](https://github.com/jackylee0424/quantum-computing-lab). We thank [@edi3on](https://github.com/edi3on) for testing and pointing out bugs.
