"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const test = require("node:test");

test("repo-local CLI packages only the canonical reviewed note and requires submit confirmation", (t) => {
  const root = path.resolve(__dirname, "..");
  const workspace = path.join(root, ".workspace");
  fs.mkdirSync(workspace, { recursive: true });
  const fixture = fs.mkdtempSync(path.join(workspace, "submission-contract-test-"));
  t.after(() => fs.rmSync(fixture, { recursive: true, force: true }));
  const oracle = path.join(fixture, "src", "shor_oracle");
  fs.mkdirSync(path.join(oracle, "memory"), { recursive: true });
  fs.writeFileSync(path.join(oracle, "field_arithmetic.rs"), "pub fn field_fixture() {}\n");
  fs.writeFileSync(path.join(oracle, "scalar_strategy.rs"), "pub fn scalar_fixture() {}\n");
  fs.writeFileSync(path.join(oracle, "architecture.mmd"), `flowchart TD
  Target["Target oracle: aP + bQ using in-place F_31 field arithmetic"]
  Algorithm["Algorithm"]
  Optimization["Optimization"]
  Target --> Algorithm
  Target --> Optimization
`);
  fs.writeFileSync(
    path.join(oracle, "memory", "README.md"),
    fs.readFileSync(path.join(root, "src", "shor_oracle", "memory", "README.md"), "utf8")
  );
  fs.writeFileSync(path.join(fixture, "ops.bin"), "operation stream");
  fs.writeFileSync(path.join(fixture, "score.json"), `${JSON.stringify({
    score: 20,
    score_model: "balanced-qubit-toffoli-depth-v1",
    metrics: { toffoli: 10, ccx: 10, ccz: 0, toffoli_depth: 10, clifford: 1, qubits: 2, ops: 1 },
    validation: {
      shots: 9024,
      gate: "fiat_shamir_shor_ecdlp_5bit_arithmetic_strategy_oracle_v2",
      checks: ["oracle correctness", "in-place F_31 field arithmetic composition", "restricted scalar strategy API", "input preservation", "phase cleanliness", "ancilla cleanup"]
    },
    artifact: "ops.bin",
    status: "ranked"
  }, null, 2)}\n`);
  fs.writeFileSync(path.join(fixture, "benchmark.json"), `${JSON.stringify({
    schemaVersion: 1,
    name: "shor-ecdlp-5bit",
    editablePaths: ["src/shor_oracle/field_arithmetic.rs", "src/shor_oracle/scalar_strategy.rs", "src/shor_oracle/architecture.mmd", "src/shor_oracle/memory"],
    scoreModel: "balanced-qubit-toffoli-depth-v1",
    scorePath: "score.json"
  }, null, 2)}\n`);

  const cli = path.join(root, "ecdlp.js");
  const run = (...args) => spawnSync(process.execPath, [cli, ...args], {
    cwd: fixture,
    encoding: "utf8",
    shell: false
  });
  const packaged = run("package", "--model", "GPT-5");
  assert.equal(packaged.status, 0, `${packaged.stdout}\n${packaged.stderr}`);
  const validated = run("validate");
  assert.equal(validated.status, 0, `${validated.stdout}\n${validated.stderr}`);

  const blockedSubmit = run("submit");
  assert.notEqual(blockedSubmit.status, 0);
  assert.match(`${blockedSubmit.stdout}\n${blockedSubmit.stderr}`, /AGENT_ACTION_REQUIRED/);
  assert.match(`${blockedSubmit.stdout}\n${blockedSubmit.stderr}`, /--confirm-docs-truthful/);

  const alternate = path.join(oracle, "memory", "alternate.md");
  fs.copyFileSync(path.join(oracle, "memory", "README.md"), alternate);
  const alternatePackage = run("package", "--model", "GPT-5", "--note-file", "src/shor_oracle/memory/alternate.md");
  assert.notEqual(alternatePackage.status, 0);
  assert.match(`${alternatePackage.stdout}\n${alternatePackage.stderr}`, /submission note must be src\/shor_oracle\/memory\/README.md/);
});
