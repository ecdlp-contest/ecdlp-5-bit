#!/usr/bin/env node
"use strict";

const crypto = require("crypto");
const fs = require("fs");
const http = require("http");
const https = require("https");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");

const DEFAULT_API = "https://ecdlp.ai";
const MAX_NOTE_BYTES = 10 * 1024;
const MAX_ARCHIVE_BYTES = 25 * 1024 * 1024;
const MAX_ARCHITECTURE_BYTES = 1024 * 1024;
const REQUIRED_SHOTS = 9024;
const SCORE_MODEL = "balanced-qubit-toffoli-depth-v1";
const REQUIRED_ARTIFACT = "ops.bin";
const SHOR_TARGET_LABEL = "Target oracle: aP + bQ using in-place F_31 field arithmetic";
const REQUIRED_ARCHITECTURE_LABELS = [SHOR_TARGET_LABEL, "Algorithm", "Optimization"];
const REQUIRED_ARCHITECTURE_PATH = "src/shor_oracle/architecture.mmd";
const FIELD_ARITHMETIC_PATH = "src/shor_oracle/field_arithmetic.rs";
const FIELD_ARITHMETIC_BANNED_PATTERNS = [
  {
    pattern: /\bQubitId\b/u,
    message: "must not name raw QubitId values; use opaque FieldInput/FieldOutput handles"
  },
  {
    pattern: /\bRegisterId\b/u,
    message: "must not observe or construct register IDs"
  },
  {
    pattern: /\bOperationType\b/u,
    message: "must not construct primitive operations directly"
  },
  {
    pattern: /\bOp\b/u,
    message: "must not construct or inspect primitive operations directly"
  },
  {
    pattern: /crate\s*::\s*circuit/u,
    message: "must not import the raw circuit module"
  },
  {
    pattern: /Signal\s*::\s*Qubit/u,
    message: "must not manufacture signals for arbitrary qubits"
  },
  {
    pattern: /\bBuilder\b/u,
    message: "must not access the trusted oracle builder"
  },
  {
    pattern: /\bunsafe\b/u,
    message: "unsafe code is not allowed in the editable field boundary"
  },
  {
    pattern: /\btransmute\b/u,
    message: "must not inspect opaque field handles by transmutation"
  },
  {
    pattern: /\bstatic\s+mut\b/u,
    message: "must not keep mutable global state across field-kernel calls"
  },
  {
    pattern: /\b(thread_local|OnceLock|LazyLock|Mutex|RwLock|Atomic[A-Za-z0-9_]*)\b/u,
    message: "must not use global state to key behavior by call order"
  },
  {
    pattern: /\b(include_bytes|include_str|std\s*::\s*fs|std\s*::\s*process|std\s*::\s*net|std\s*::\s*env)\b/u,
    message: "must not load external data or depend on process/environment state"
  }
];

const TRACKS = {
  "point-double-secp256k1-v1": {
    gate: "fiat_shamir_point_double",
    editablePaths: ["src/point_double"],
    requiredChecks: ["classical correctness", "input preservation", "phase cleanliness", "ancilla cleanup"],
    defaultNoteFile: "src/point_double/memory/README.md"
  },
  "shor-ecdlp-5bit": {
    gate: "fiat_shamir_shor_ecdlp_5bit_in_place_field_arithmetic_oracle_v1",
    editablePaths: ["src/shor_oracle/field_arithmetic.rs", "src/shor_oracle/architecture.mmd", "src/shor_oracle/memory"],
    requiredChecks: ["oracle correctness", "in-place F_31 field arithmetic composition", "input preservation", "phase cleanliness", "ancilla cleanup"],
    defaultNoteFile: "src/shor_oracle/memory/README.md",
    architectureDiagram: REQUIRED_ARCHITECTURE_PATH
  },
  "shor-ecdlp-7bit-v1": {
    gate: "fiat_shamir_shor_ecdlp_7bit_variable_q_oracle",
    editablePaths: ["src/shor_oracle"],
    requiredChecks: ["oracle correctness", "input preservation", "phase cleanliness", "ancilla cleanup"],
    defaultNoteFile: "src/shor_oracle/memory/README.md",
    architectureDiagram: REQUIRED_ARCHITECTURE_PATH
  }
};

const VALUE_FLAGS = new Set([
  "--api",
  "--archive",
  "--claimed-score",
  "--manifest",
  "--model",
  "--note",
  "--note-file",
  "--out",
  "--poll-interval",
  "--source-url",
  "--timeout",
  "--track"
]);

const HELP_TEXT = {
  main: `ecdlp contest CLI

Usage:
  ecdlp <command> [options]
  ./ecdlp.js <command> [options]

Commands:
  setup        Install or prepare local benchmark dependencies
  run          Build and score the local oracle implementation
  package      Create dist/submission.tar.gz and submission metadata
  validate     Check a local package against the contest contract
  submit       Upload a validated package to ${DEFAULT_API}
  login        Save a contest API key locally
  config       Show the active API endpoint and token status
  status       Show or watch a submission status
  logs         Print server-side validation logs
  leaderboard  Show accepted submissions for a track

Help:
  ecdlp <command> --help
  ./ecdlp.js <command> --help

Agent guidance:
  Read README.md and benchmark.json first. Use command-level --help before each
  step. Contestant code edits should stay in src/shor_oracle/field_arithmetic.rs.
  Keep notes under src/shor_oracle/memory/ and update src/shor_oracle/architecture.mmd.
  The trusted builder owns raw qubits, primitive ops, register allocation, and
  segment boundaries.
  Do not hand-edit score.json, ops.bin, results.tsv, or the trusted harness.

Use repo-local build, cache, scratch, and tool paths under .workspace/ to avoid
permission issues. Cargo is configured to build into .workspace/target.

Local work does not require an API key. Submitting does: the user must sign in
with GitHub at ${DEFAULT_API}/account, create an API key, and run ecdlp login.`,

  setup: `ecdlp setup

Usage:
  ecdlp setup
  ./ecdlp.js setup

Runs benchmark.json setupCommand from the repository root.

Setup and later builds should use repo-local paths under .workspace/. The
checked-in Cargo config routes Rust build artifacts to .workspace/target.

Start here when the repo has not been prepared on this machine. After setup,
run:
  ecdlp run --help`,

  run: `ecdlp run

Usage:
  ecdlp run [--note "short note"]
  ./ecdlp.js run [--note "short note"]

Runs benchmark.json benchmarkCommand. The trusted evaluator writes:
  ops.bin
  score.json
  results.tsv

Build artifacts should stay under .workspace/target. Put any extra scratch
outputs or generated experiments under .workspace/ so they remain ignored by
git and avoid system permission issues.

Use this after modifying src/shor_oracle/field_arithmetic.rs or its notes/diagram.
A valid run must pass all 9024
Fiat-Shamir shots and produce score.json with status "ranked".

This command is local and does not require an API key.`,

  package: `ecdlp package

Usage:
  ecdlp package --model MODEL [--note-file PATH] [--out dist]
  ./ecdlp.js package --model MODEL [--note-file PATH] [--out dist]

Creates:
  dist/submission.tar.gz
  dist/submission-note.md
  dist/submission-metadata.json

Requirements:
  - score.json must be ranked from a trusted local run.
  - --model is required.
  - the note must be non-empty and at most 10 KiB after the Model prefix.
  - editable paths must match the track boundary.
  - src/shor_oracle/architecture.mmd must satisfy the Mermaid diagram contract.

Example:
  ecdlp package --note-file src/shor_oracle/memory/README.md --model "GPT-5"

This command is local and does not require an API key.`,

  validate: `ecdlp validate

Usage:
  ecdlp validate [dist/submission-metadata.json] [--track TRACK_ID]
  ./ecdlp.js validate [dist/submission-metadata.json] [--track TRACK_ID]

Checks the local package metadata, archive, artifact hash, score formula,
validation record, editable-path boundary, and architecture diagram commitment.

Run this before submit. A package that fails local validation should not be
uploaded.

This command is local and does not require an API key.`,

  submit: `ecdlp submit

Usage:
  ecdlp submit [dist/submission-metadata.json] [--source-url URL] [--watch] [--api ${DEFAULT_API}]
  ./ecdlp.js submit [dist/submission-metadata.json] [--source-url URL] [--watch] [--api ${DEFAULT_API}]

Validates the local package, checks the current leaderboard, and uploads only if
the local score is strictly better than the best ranked score for the track.

Submitting requires authentication. The user must sign in with GitHub at
${DEFAULT_API}/account, create an API key, and run:
  ecdlp login <api-key>

Options:
  --source-url URL       Public source or pull request URL for reviewer context
  --watch               Poll until the server reaches a terminal state
  --poll-interval SEC   Polling interval for --watch, default 10
  --timeout SEC         Maximum watch time, 0 means no timeout
  --api URL             API endpoint, default ${DEFAULT_API}

Before submitting:
  1. Run ecdlp validate.
  2. Make sure src/shor_oracle/memory/README.md explains the field-kernel approach.
  3. Make sure src/shor_oracle/architecture.mmd matches the submitted arithmetic implementation.`,

  login: `ecdlp login

Usage:
  ecdlp login <api-key> [--api ${DEFAULT_API}]
  ./ecdlp.js login <api-key> [--api ${DEFAULT_API}]

Submitting requires authentication. Open ${DEFAULT_API}/account, sign in with
GitHub, create an API key, then run this command. The key is saved in the local
ecdlp config file.

Environment alternatives:
  ECDLP_API_TOKEN
  ECDLP_API_KEY`,

  config: `ecdlp config

Usage:
  ecdlp config [--api ${DEFAULT_API}]
  ./ecdlp.js config [--api ${DEFAULT_API}]

Shows the active API endpoint, whether a token is configured, and the config
file path.`,

  status: `ecdlp status

Usage:
  ecdlp status <submission-id> [--watch] [--poll-interval 10] [--timeout 0]
  ./ecdlp.js status <submission-id> [--watch] [--poll-interval 10] [--timeout 0]

Prints server status, rank status, score, trusted-worker state, and merge
metadata when available. Use --watch after submit to follow validation.`,

  logs: `ecdlp logs

Usage:
  ecdlp logs <submission-id>
  ./ecdlp.js logs <submission-id>

Prints server-side validation logs for a submission. Use this to distinguish
review-pending, trusted-worker, duplicate, and failed-validation states.`,

  leaderboard: `ecdlp leaderboard

Usage:
  ecdlp leaderboard [--track TRACK_ID]
  ./ecdlp.js leaderboard [--track TRACK_ID]

Shows accepted ranked submissions for the current benchmark track. The submit
command uses the same leaderboard to reject equal-or-worse local packages before
uploading.`
};

function hasFlag(args, name) {
  return args.includes(name);
}

function numberFlag(args, name, fallback) {
  const raw = getFlag(args, name, null);
  if (raw === null) return fallback;
  const value = Number(raw);
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(`${name} must be a non-negative number`);
  }
  return value;
}

function isHelpFlag(value) {
  return value === "--help" || value === "-h";
}

function usage(exitCode = 0, command = "main") {
  console.log(HELP_TEXT[command] || HELP_TEXT.main);
  process.exit(exitCode);
}

function configPath() {
  if (process.env.ECDLP_CONFIG) return process.env.ECDLP_CONFIG;
  const base = process.env.APPDATA || path.join(os.homedir(), ".config");
  return path.join(base, "ecdlp", "config.json");
}

function readConfig() {
  try {
    return JSON.parse(fs.readFileSync(configPath(), "utf8"));
  } catch {
    return {};
  }
}

function writeConfig(config) {
  const target = configPath();
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, `${JSON.stringify(config, null, 2)}\n`, { mode: 0o600 });
}

function apiUrl(args = []) {
  return (getFlag(args, "--api") || process.env.ECDLP_API_URL || readConfig().api_url || DEFAULT_API).replace(/\/$/, "");
}

function apiToken() {
  return process.env.ECDLP_API_TOKEN || process.env.ECDLP_API_KEY || readConfig().api_token || "";
}

function authHeaders() {
  const token = apiToken();
  return token ? { authorization: `Bearer ${token}` } : {};
}

function getFlag(args, name, fallback = null) {
  const index = args.indexOf(name);
  if (index === -1) return fallback;
  return args[index + 1] || fallback;
}

function firstPositional(args) {
  for (let index = 0; index < args.length; index += 1) {
    const value = args[index];
    if (VALUE_FLAGS.has(value)) {
      index += 1;
      continue;
    }
    if (!value.startsWith("-")) return value;
  }
  return null;
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(path.resolve(filePath), "utf8"));
}

function repoManifest(manifestPath = "benchmark.json") {
  const filePath = path.resolve(manifestPath);
  if (!fs.existsSync(filePath)) {
    throw new Error(`${manifestPath} not found; run inside a contest baseline repo`);
  }
  const manifest = readJson(filePath);
  if (manifest.schemaVersion !== 1) throw new Error("benchmark.json schemaVersion must be 1");
  if (!manifest.name || !TRACKS[manifest.name]) throw new Error(`unsupported benchmark '${manifest.name}'`);
  return manifest;
}

function configuredTargetDirEnv() {
  if (process.env.CARGO_TARGET_DIR) return {};
  const cargoConfig = path.resolve(".cargo", "config.toml");
  if (!fs.existsSync(cargoConfig)) return {};
  const text = fs.readFileSync(cargoConfig, "utf8");
  const match = text.match(/^\s*target-dir\s*=\s*["']([^"']+)["']/m);
  return match ? { CARGO_TARGET_DIR: match[1] } : {};
}

function runManifestCommand(field, extraArgs = []) {
  const manifest = repoManifest();
  const command = manifest[field];
  if (!Array.isArray(command) || command.length === 0) throw new Error(`benchmark.json ${field} is missing`);
  const [program, ...args] = command;
  const finalArgs = program === "bash" && args[0] === "-lc" && extraArgs.length > 0
    ? ["-lc", `${args[1]} "$@"`, "_", ...args.slice(2), ...extraArgs]
    : [...args, ...extraArgs];
  console.log(`> ${[program, ...finalArgs].join(" ")}`);
  const result = spawnSync(program, finalArgs, {
    cwd: process.cwd(),
    env: { ...process.env, ...configuredTargetDirEnv() },
    stdio: "inherit",
    shell: false
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${field} failed with exit code ${result.status}`);
}

function normalizeRepoPath(value) {
  return String(value || "").replace(/\\/g, "/").replace(/^\/+|\/+$/g, "");
}

function assertRepoRelativePath(repoPath, fieldName) {
  const normalized = normalizeRepoPath(repoPath);
  if (!normalized) throw new Error(`${fieldName} must not be empty`);
  if (path.isAbsolute(repoPath)) throw new Error(`${fieldName} must be repo-relative: ${repoPath}`);
  if (normalized.split("/").includes("..")) throw new Error(`${fieldName} must not contain '..': ${repoPath}`);
  if (normalized === "benchmark.json") throw new Error(`${fieldName} must not be benchmark.json`);
  return normalized;
}

function sha256File(filePath) {
  const hash = crypto.createHash("sha256");
  hash.update(fs.readFileSync(filePath));
  return hash.digest("hex");
}

function listArchiveEntries(archivePath) {
  return listArchiveEntriesDetailed(archivePath).map((entry) => entry.normalized);
}

function listArchiveEntriesDetailed(archivePath) {
  const result = spawnSync("tar", ["-tzf", archivePath], {
    cwd: process.cwd(),
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    shell: false
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`tar -tzf failed with exit code ${result.status}`);
  return result.stdout
    .split(/\r?\n/)
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((raw) => ({ raw, normalized: normalizeRepoPath(raw) }));
}

function isArchiveEntryInEditableScope(entry, editablePaths) {
  return editablePaths.some((editablePath) => (
    entry === editablePath ||
    entry.startsWith(`${editablePath}/`) ||
    editablePath.startsWith(`${entry}/`)
  ));
}

function archivePackageErrors(spec, metadataPath, metadata) {
  if (!metadataPath || !metadata?.archive) return [];
  const errors = [];
  const archivePath = path.resolve(path.dirname(path.resolve(metadataPath)), metadata.archive);
  if (!fs.existsSync(archivePath)) {
    errors.push(`${metadata.archive} is missing beside ${path.basename(metadataPath)}`);
    return errors;
  }

  const stat = fs.statSync(archivePath);
  if (Number.isInteger(metadata.archiveBytes) && metadata.archiveBytes !== stat.size) {
    errors.push(`archiveBytes does not match local ${metadata.archive}`);
  }

  let entries;
  try {
    entries = listArchiveEntriesDetailed(archivePath);
  } catch (error) {
    errors.push(`could not inspect ${metadata.archive}: ${error.message}`);
    return errors;
  }
  if (entries.length === 0) errors.push(`${metadata.archive} must not be empty`);

  const editablePaths = (spec?.editablePaths || []).map(normalizeRepoPath);
  for (const entry of entries) {
    if (entry.raw.startsWith("/") || entry.normalized.split("/").includes("..")) {
      errors.push(`${metadata.archive} contains unsafe entry: ${entry.raw}`);
      continue;
    }
    if (!isArchiveEntryInEditableScope(entry.normalized, editablePaths)) {
      errors.push(`${metadata.archive} contains entry outside editable paths: ${entry.normalized}`);
    }
  }
  return errors;
}

function stripMermaidComments(text) {
  return text
    .split(/\r?\n/)
    .map((line) => line.replace(/%%.*$/u, "").trim())
    .filter(Boolean);
}

function parseMermaidNodeToken(token, idsByLabel) {
  const match = token.trim().match(/^([A-Za-z][\w-]*)(.*)$/u);
  if (!match) return null;
  const id = match[1];
  const rest = match[2] || "";
  const quoted = rest.match(/^[\s]*(?:\[|\(|\{)\s*"([^"]+)"\s*(?:\]|\)|\})/u);
  const bare = rest.match(/^[\s]*(?:\[|\(|\{)\s*([^\]\)\}]+?)\s*(?:\]|\)|\})/u);
  const label = (quoted?.[1] || bare?.[1] || "").trim();
  if (label) {
    if (!idsByLabel.has(label)) idsByLabel.set(label, new Set());
    idsByLabel.get(label).add(id);
  }
  return id;
}

function inspectMermaidArchitecture(text) {
  const lines = stripMermaidComments(text);
  const errors = [];
  if (!/^(flowchart|graph)\s+(TD|TB|BT|LR|RL)\b/u.test(lines[0] || "")) {
    errors.push("diagram must start with a Mermaid flowchart or graph declaration");
  }

  const idsByLabel = new Map();
  const edges = [];
  for (const line of lines.slice(1)) {
    const compactArrow = line.match(/^(.*?)\s*(?:-->|---?>|==>)\s*(.*)$/u);
    if (compactArrow) {
      const from = parseMermaidNodeToken(compactArrow[1], idsByLabel);
      const to = parseMermaidNodeToken(compactArrow[2], idsByLabel);
      if (from && to) edges.push([from, to]);
      continue;
    }
    parseMermaidNodeToken(line, idsByLabel);
  }

  for (const label of REQUIRED_ARCHITECTURE_LABELS) {
    if (!idsByLabel.has(label)) errors.push(`diagram must contain exact anchor label '${label}'`);
  }

  const targetIds = idsByLabel.get(SHOR_TARGET_LABEL) || new Set();
  const algorithmIds = idsByLabel.get("Algorithm") || new Set();
  const optimizationIds = idsByLabel.get("Optimization") || new Set();
  const hasEdge = (fromIds, toIds) => edges.some(([from, to]) => fromIds.has(from) && toIds.has(to));
  if (targetIds.size && algorithmIds.size && !hasEdge(targetIds, algorithmIds)) {
    errors.push(`${SHOR_TARGET_LABEL} must have an outgoing edge to Algorithm`);
  }
  if (targetIds.size && optimizationIds.size && !hasEdge(targetIds, optimizationIds)) {
    errors.push(`${SHOR_TARGET_LABEL} must have an outgoing edge to Optimization`);
  }
  return errors;
}

function architectureDiagramErrors(spec, metadataPath = null, metadata = null) {
  const diagramPath = spec?.architectureDiagram;
  if (!diagramPath) return [];
  const errors = [];
  const absolutePath = path.resolve(diagramPath);
  if (!fs.existsSync(absolutePath)) {
    errors.push(`${diagramPath} is required`);
    return errors;
  }
  const stat = fs.statSync(absolutePath);
  if (!stat.isFile()) errors.push(`${diagramPath} must be a file`);
  if (stat.size <= 0 || stat.size > MAX_ARCHITECTURE_BYTES) {
    errors.push(`${diagramPath} must be between 1 and ${MAX_ARCHITECTURE_BYTES} bytes`);
  }
  const text = fs.readFileSync(absolutePath, "utf8");
  if (text.includes("\uFFFD")) errors.push(`${diagramPath} must be valid UTF-8 text`);
  errors.push(...inspectMermaidArchitecture(text).map((message) => `${diagramPath}: ${message}`));

  if (metadata && (!metadata.architectureDiagram || typeof metadata.architectureDiagram !== "object" || Array.isArray(metadata.architectureDiagram))) {
    errors.push("metadata.architectureDiagram is required");
  } else if (metadata) {
    const commitment = metadata.architectureDiagram;
    const digest = sha256File(absolutePath);
    if (commitment.path !== diagramPath) {
      errors.push(`metadata.architectureDiagram.path must be ${diagramPath}`);
    }
    if (!Number.isInteger(commitment.bytes) || commitment.bytes <= 0 || commitment.bytes > MAX_ARCHITECTURE_BYTES) {
      errors.push(`metadata.architectureDiagram.bytes must be between 1 and ${MAX_ARCHITECTURE_BYTES}`);
    } else if (commitment.bytes !== stat.size) {
      errors.push(`metadata.architectureDiagram.bytes does not match local ${diagramPath}`);
    }
    if (typeof commitment.sha256 !== "string" || !/^[0-9a-f]{64}$/i.test(commitment.sha256)) {
      errors.push("metadata.architectureDiagram.sha256 must be a 64-character SHA-256 hex digest");
    } else if (commitment.sha256.toLowerCase() !== digest) {
      errors.push(`metadata.architectureDiagram.sha256 does not match local ${diagramPath}`);
    }
  }

  if (metadataPath && metadata?.archive) {
    const archivePath = path.resolve(path.dirname(path.resolve(metadataPath)), metadata.archive);
    if (fs.existsSync(archivePath)) {
      try {
        const entries = listArchiveEntries(archivePath);
        if (!entries.includes(normalizeRepoPath(diagramPath))) {
          errors.push(`${metadata.archive} must include ${diagramPath}`);
        }
      } catch (error) {
        errors.push(`could not inspect ${metadata.archive}: ${error.message}`);
      }
    }
  }
  return errors;
}

function assertArchitectureDiagram(spec) {
  const errors = architectureDiagramErrors(spec);
  if (errors.length > 0) throw new Error(errors[0]);
}

function firstLineMatching(text, pattern) {
  const lines = text.split(/\r?\n/u);
  for (let i = 0; i < lines.length; i += 1) {
    if (pattern.test(lines[i])) {
      pattern.lastIndex = 0;
      return i + 1;
    }
    pattern.lastIndex = 0;
  }
  return 1;
}

function fieldArithmeticSourceErrors() {
  const absolutePath = path.resolve(FIELD_ARITHMETIC_PATH);
  if (!fs.existsSync(absolutePath)) return [`${FIELD_ARITHMETIC_PATH} is required`];
  const stat = fs.statSync(absolutePath);
  if (!stat.isFile()) return [`${FIELD_ARITHMETIC_PATH} must be a file`];
  const text = fs.readFileSync(absolutePath, "utf8");
  const errors = [];
  for (const { pattern, message } of FIELD_ARITHMETIC_BANNED_PATTERNS) {
    if (pattern.test(text)) {
      const line = firstLineMatching(text, pattern);
      errors.push(`${FIELD_ARITHMETIC_PATH}:${line}: ${message}`);
      pattern.lastIndex = 0;
    }
  }
  return errors;
}

function assertFieldArithmeticSourceBoundary() {
  const errors = fieldArithmeticSourceErrors();
  if (errors.length > 0) throw new Error(errors[0]);
}

function utf8Bytes(text) {
  return Buffer.byteLength(text, "utf8");
}

function sameStringArray(left, right) {
  if (left.length !== right.length) return false;
  return left.every((value, index) => value === right[index]);
}

function scoresMatch(left, right) {
  if (!Number.isFinite(left) || !Number.isFinite(right)) return false;
  return Math.abs(left - right) <= Number.EPSILON * Math.max(1, Math.abs(left), Math.abs(right)) * 8;
}

function packageSubmission(args) {
  const manifest = repoManifest(getFlag(args, "--manifest", "benchmark.json"));
  const spec = TRACKS[manifest.name];
  if (manifest.scoreModel !== SCORE_MODEL) throw new Error(`benchmark.json scoreModel must be ${SCORE_MODEL}`);
  if (manifest.scorePath !== "score.json") throw new Error("benchmark.json scorePath must be score.json");

  const editablePaths = Array.isArray(manifest.editablePaths) ? manifest.editablePaths.map((item) => assertRepoRelativePath(item, "editablePaths")) : [];
  const expectedEditablePaths = spec.editablePaths.map(normalizeRepoPath);
  if (!sameStringArray(editablePaths.slice().sort(), expectedEditablePaths.slice().sort())) {
    throw new Error(`editablePaths must be exactly ${expectedEditablePaths.join(", ")}`);
  }
  for (const editablePath of editablePaths) {
    if (!fs.existsSync(path.resolve(editablePath))) throw new Error(`editable path does not exist: ${editablePath}`);
  }
  assertArchitectureDiagram(spec);
  assertFieldArithmeticSourceBoundary();
  const architecturePath = spec.architectureDiagram;
  const architectureBytes = fs.statSync(path.resolve(architecturePath)).size;
  const architectureSha256 = sha256File(path.resolve(architecturePath));

  const model = getFlag(args, "--model", "");
  if (!model.trim()) throw new Error("--model is required");
  const noteFile = getFlag(args, "--note-file", spec.defaultNoteFile);
  if (!fs.existsSync(path.resolve(noteFile))) throw new Error(`note file not found: ${noteFile}`);
  const rawNote = fs.readFileSync(path.resolve(noteFile), "utf8");
  if (!rawNote.trim()) throw new Error("submission note must not be empty");
  const submissionNote = `Model: ${model.trim()}\n\n${rawNote}`;
  const noteBytes = utf8Bytes(submissionNote);
  if (noteBytes > MAX_NOTE_BYTES) throw new Error(`submission note must be at most ${MAX_NOTE_BYTES} bytes (${noteBytes} bytes provided)`);

  const score = readJson(manifest.scorePath);
  if (score.status !== "ranked") throw new Error("score.json status is not ranked");
  if (score.validation?.shots !== REQUIRED_SHOTS || score.validation?.gate !== spec.gate) {
    throw new Error(`score.json does not show the required ${REQUIRED_SHOTS}-shot ${spec.gate} gate`);
  }
  if (score.score_model !== SCORE_MODEL) throw new Error(`score.json score_model must be ${SCORE_MODEL}`);
  if (score.artifact !== REQUIRED_ARTIFACT) throw new Error(`score.json artifact must be ${REQUIRED_ARTIFACT}`);
  for (const metricName of ["toffoli", "ccx", "ccz", "toffoli_depth", "clifford", "qubits", "ops"]) {
    if (!Number.isFinite(score.metrics?.[metricName]) || score.metrics[metricName] < 0) {
      throw new Error(`score.json metrics.${metricName} is missing or invalid`);
    }
  }

  const artifactPath = path.resolve(score.artifact);
  if (!fs.existsSync(artifactPath)) throw new Error(`score.json artifact is missing: ${score.artifact}`);
  const artifactBytes = fs.statSync(artifactPath).size;
  if (artifactBytes <= 0) throw new Error(`score.json artifact must not be empty: ${score.artifact}`);
  const artifactSha256 = sha256File(artifactPath);

  const outDir = getFlag(args, "--out", "dist");
  fs.mkdirSync(path.resolve(outDir), { recursive: true });
  const archivePath = path.resolve(outDir, "submission.tar.gz");
  const notePath = path.resolve(outDir, "submission-note.md");
  const metadataPath = path.resolve(outDir, "submission-metadata.json");
  try { fs.unlinkSync(archivePath); } catch {}

  const tar = spawnSync("tar", ["-czf", archivePath, "-C", process.cwd(), ...editablePaths], { stdio: "inherit", shell: false });
  if (tar.error) throw tar.error;
  if (tar.status !== 0) throw new Error(`tar failed with exit code ${tar.status}`);
  const archiveBytes = fs.statSync(archivePath).size;
  if (archiveBytes > MAX_ARCHIVE_BYTES) throw new Error(`submission archive must be at most ${MAX_ARCHIVE_BYTES} bytes (${archiveBytes} bytes produced)`);

  fs.writeFileSync(notePath, submissionNote, "utf8");
  const metadata = {
    schemaVersion: 1,
    benchmark: manifest.name,
    editablePaths,
    archive: "submission.tar.gz",
    archiveBytes,
    note: "submission-note.md",
    noteBytes,
    model: model.trim(),
    claimedScore: getFlag(args, "--claimed-score") ? Number(getFlag(args, "--claimed-score")) : null,
    localScore: score.score,
    scoreModel: score.score_model,
    metrics: score.metrics,
    validation: score.validation,
    artifact: score.artifact,
    artifactBytes,
    artifactSha256,
    architectureDiagram: architecturePath ? {
      path: architecturePath,
      bytes: architectureBytes,
      sha256: architectureSha256
    } : undefined,
    generatedAt: new Date().toISOString()
  };
  fs.writeFileSync(metadataPath, `${JSON.stringify(metadata, null, 2)}\n`, "utf8");

  console.log(`Packaged editable paths: ${editablePaths.join(", ")}`);
  console.log(`Archive: ${path.relative(process.cwd(), archivePath)} (${archiveBytes} bytes)`);
  console.log(`Artifact: ${score.artifact} (${artifactBytes} bytes, sha256 ${artifactSha256})`);
  console.log(`Note: ${path.relative(process.cwd(), notePath)} (${noteBytes} bytes)`);
  console.log(`Metadata: ${path.relative(process.cwd(), metadataPath)}`);
}

function defaultSubmissionPath() {
  for (const candidate of [path.resolve("dist", "submission-metadata.json"), path.resolve("submission-metadata.json")]) {
    if (fs.existsSync(candidate)) return candidate;
  }
  throw new Error("submission metadata not found; run ./ecdlp.js package or pass a metadata path");
}

function validatePackage(metadata, options = {}) {
  const logs = [];
  const error = (code, message) => logs.push({ level: "error", code, message });
  const info = (code, message) => logs.push({ level: "info", code, message });
  const benchmark = options.trackId || metadata?.benchmark;
  const spec = TRACKS[benchmark];
  if (!spec) error("PACKAGE_BENCHMARK_UNKNOWN", `unsupported benchmark '${benchmark || ""}'`);

  if (!metadata || typeof metadata !== "object" || Array.isArray(metadata)) {
    error("PACKAGE_ROOT", "submission metadata must be a JSON object");
  }
  if (metadata.schemaVersion !== 1) error("PACKAGE_SCHEMA_VERSION", "schemaVersion must be 1");
  if (spec && metadata.benchmark !== benchmark) error("PACKAGE_BENCHMARK", `benchmark must be ${benchmark}`);

  const expectedEditablePaths = spec ? spec.editablePaths.map(normalizeRepoPath) : [];
  const editablePaths = Array.isArray(metadata.editablePaths) ? metadata.editablePaths.map(normalizeRepoPath) : [];
  if (!sameStringArray(editablePaths.slice().sort(), expectedEditablePaths.slice().sort())) {
    error("PACKAGE_EDITABLE_PATHS", `editablePaths must be exactly ${expectedEditablePaths.join(", ")}`);
  }
  if (metadata.archive !== "submission.tar.gz") error("PACKAGE_ARCHIVE", "archive must be submission.tar.gz");
  if (!Number.isInteger(metadata.archiveBytes) || metadata.archiveBytes <= 0 || metadata.archiveBytes > MAX_ARCHIVE_BYTES) {
    error("PACKAGE_ARCHIVE_BYTES", `archiveBytes must be between 1 and ${MAX_ARCHIVE_BYTES}`);
  }
  if (metadata.note !== "submission-note.md") error("PACKAGE_NOTE", "note must be submission-note.md");
  if (!Number.isInteger(metadata.noteBytes) || metadata.noteBytes <= 0 || metadata.noteBytes > MAX_NOTE_BYTES) {
    error("PACKAGE_NOTE_BYTES", `noteBytes must be between 1 and ${MAX_NOTE_BYTES}`);
  }
  if (typeof metadata.model !== "string" || !metadata.model.trim()) error("PACKAGE_MODEL", "model must be a non-empty string");
  if (metadata.scoreModel !== SCORE_MODEL) error("PACKAGE_SCORE_MODEL", `scoreModel must be ${SCORE_MODEL}`);

  const metrics = metadata.metrics || {};
  for (const metricName of ["toffoli", "ccx", "ccz", "toffoli_depth", "clifford", "qubits", "ops"]) {
    if (!Number.isFinite(metrics[metricName]) || metrics[metricName] < 0) error("PACKAGE_METRIC", `metrics.${metricName} must be a non-negative finite number`);
  }
  const score = Math.round(Number(metrics.qubits || 0)) * Math.sqrt(Math.round(Number(metrics.toffoli || 0)) * Math.round(Number(metrics.toffoli_depth || 0)));
  if (!scoresMatch(Number(metadata.localScore), score)) {
    error("PACKAGE_SCORE", `localScore must equal metrics.qubits * sqrt(round(metrics.toffoli) * round(metrics.toffoli_depth)) (${score})`);
  }

  if (metadata.validation?.shots !== REQUIRED_SHOTS) error("PACKAGE_VALIDATION_SHOTS", `validation.shots must be ${REQUIRED_SHOTS}`);
  if (spec && metadata.validation?.gate !== spec.gate) error("PACKAGE_VALIDATION_GATE", `validation.gate must be ${spec.gate}`);
  const checks = Array.isArray(metadata.validation?.checks) ? metadata.validation.checks : [];
  for (const required of spec?.requiredChecks || []) {
    if (!checks.includes(required)) error("PACKAGE_VALIDATION_CHECK", `validation.checks must include '${required}'`);
  }
  if (metadata.artifact !== REQUIRED_ARTIFACT) error("PACKAGE_ARTIFACT", `artifact must be ${REQUIRED_ARTIFACT}`);
  if (!Number.isInteger(metadata.artifactBytes) || metadata.artifactBytes <= 0) error("PACKAGE_ARTIFACT_BYTES", "artifactBytes must be a positive integer");
  if (typeof metadata.artifactSha256 !== "string" || !/^[0-9a-f]{64}$/i.test(metadata.artifactSha256)) {
    error("PACKAGE_ARTIFACT_SHA256", "artifactSha256 must be a 64-character SHA-256 hex digest");
  }

  if (metadata.artifact && fs.existsSync(path.resolve(metadata.artifact))) {
    const stat = fs.statSync(path.resolve(metadata.artifact));
    const digest = sha256File(path.resolve(metadata.artifact));
    if (metadata.artifactBytes !== stat.size) error("PACKAGE_ARTIFACT_BYTES", `artifactBytes does not match local ${metadata.artifact}`);
    if (metadata.artifactSha256 && metadata.artifactSha256.toLowerCase() !== digest) error("PACKAGE_ARTIFACT_SHA256", `artifactSha256 does not match local ${metadata.artifact}`);
  }

  for (const message of architectureDiagramErrors(spec, options.metadataPath || null, metadata)) {
    error("PACKAGE_ARCHITECTURE_DIAGRAM", message);
  }
  for (const message of fieldArithmeticSourceErrors()) {
    error("FIELD_ARITHMETIC_BOUNDARY", message);
  }
  for (const message of archivePackageErrors(spec, options.metadataPath || null, metadata)) {
    error("PACKAGE_ARCHIVE", message);
  }

  if (!logs.some((entry) => entry.level === "error")) {
    info("PACKAGE_OK", "submission metadata matches baseline package contract");
    info("METRICS_OK", `score=${score}`);
  }
  return { ok: !logs.some((entry) => entry.level === "error"), logs, score, trackId: benchmark };
}

function printValidation(result) {
  console.log(`track: ${result.trackId || "unknown"}`);
  for (const entry of result.logs) console.log(`${entry.level.toUpperCase()} ${entry.code}: ${entry.message}`);
  if (result.ok) console.log(`score: ${result.score}`);
  console.log(`status: ${result.ok ? "ranked" : "failed"}`);
}

function printSubmissionStatus(response) {
  console.log(`submission_id: ${response.submission_id || response.id}`);
  console.log(`track: ${response.track_id}`);
  console.log(`server_status: ${response.status}`);
  console.log(`rank_status: ${response.rank_status}`);
  if (response.metrics?.score !== undefined) console.log(`score: ${response.metrics.score}`);
  if (response.failure_code) console.log(`failure_code: ${response.failure_code}`);
  if (response.accepted_by_github_login) console.log(`accepted_by: @${response.accepted_by_github_login}`);
  if (response.trusted_worker_passed_at) console.log(`trusted_worker_passed_at: ${response.trusted_worker_passed_at}`);
  if (response.merge_url) console.log(`merge_url: ${response.merge_url}`);
  if (response.merge_commit_sha) console.log(`merge_commit_sha: ${response.merge_commit_sha}`);
}

async function assertScoreImprovesLeaderboard(trackId, localScore, args = []) {
  const response = await requestJson(`${apiUrl(args)}/api/leaderboard?track_id=${encodeURIComponent(trackId)}`);
  const rows = Array.isArray(response.rows) ? response.rows : [];
  const best = rows
    .filter((row) => Number.isFinite(Number(row.score)))
    .sort((left, right) => Number(left.score) - Number(right.score))[0];
  if (!best) {
    console.log("score_gate: no ranked submissions yet");
    return;
  }
  const bestScore = Number(best.score);
  if (localScore >= bestScore) {
    const id = best.submission_id || best.id || "unknown";
    throw new Error(`local score ${localScore} is not better than current best ${bestScore} for ${trackId} (${id})`);
  }
  console.log(`score_gate: local score ${localScore} beats current best ${bestScore}`);
}

function isTerminalSubmission(response) {
  return response.status === "ranked" || response.status === "failed";
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function readNoteOption(args, filePath, metadata) {
  const note = getFlag(args, "--note");
  const noteFile = getFlag(args, "--note-file");
  if (note && noteFile) throw new Error("pass either --note or --note-file, not both");
  if (noteFile) return fs.readFileSync(path.resolve(noteFile), "utf8");
  if (!note && metadata.note) {
    const candidate = path.resolve(path.dirname(path.resolve(filePath)), metadata.note);
    if (fs.existsSync(candidate)) return fs.readFileSync(candidate, "utf8");
  }
  return note || "";
}

function readArchiveInfo(args, filePath, metadata) {
  const archiveFlag = getFlag(args, "--archive");
  const archivePath = archiveFlag
    ? path.resolve(archiveFlag)
    : metadata.archive
      ? path.resolve(path.dirname(path.resolve(filePath)), metadata.archive)
      : null;
  if (!archivePath || !fs.existsSync(archivePath)) return {};
  return {
    archive_sha256: sha256File(archivePath),
    archive_size_bytes: fs.statSync(archivePath).size,
    archive_base64: fs.readFileSync(archivePath).toString("base64")
  };
}

function nodeFetch(url, options = {}, redirects = 0) {
  return new Promise((resolve, reject) => {
    const target = new URL(url);
    const client = target.protocol === "http:" ? http : https;
    const request = client.request(target, {
      method: options.method || "GET",
      headers: options.headers || {}
    }, (response) => {
      const chunks = [];
      response.on("data", (chunk) => chunks.push(chunk));
      response.on("end", async () => {
        const body = Buffer.concat(chunks).toString("utf8");
        if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
          if (redirects >= 5) {
            reject(new Error("too many redirects"));
            return;
          }
          const nextUrl = new URL(response.headers.location, target).toString();
          try {
            resolve(await nodeFetch(nextUrl, options, redirects + 1));
          } catch (error) {
            reject(error);
          }
          return;
        }
        resolve({
          ok: response.statusCode >= 200 && response.statusCode < 300,
          status: response.statusCode,
          text: async () => body
        });
      });
    });
    request.on("error", reject);
    if (options.body) request.write(options.body);
    request.end();
  });
}

async function requestJson(url, options = {}) {
  const response = await nodeFetch(url, {
    ...options,
    headers: { "content-type": "application/json", ...(options.headers || {}) }
  });
  const text = await response.text();
  const json = text ? JSON.parse(text) : null;
  if (!response.ok) throw new Error(json?.error || `HTTP ${response.status}`);
  return json;
}

async function login(token, args) {
  if (!token) {
    console.log(`Open ${apiUrl(args)}/account, sign in with GitHub, create an API key, then run:`);
    console.log("./ecdlp.js login <api-key>");
    process.exit(1);
  }
  const targetApi = apiUrl(args);
  const response = await requestJson(`${targetApi}/api/me`, { headers: { authorization: `Bearer ${token}` } });
  writeConfig({ ...readConfig(), api_url: targetApi, api_token: token });
  console.log(`logged in: @${response.user.github_login}`);
  console.log(`api: ${targetApi}`);
  console.log(`config: ${configPath()}`);
}

function showConfig(args) {
  const token = apiToken();
  console.log(`api: ${apiUrl(args)}`);
  console.log(`token: ${token ? `${token.slice(0, 12)}...${token.slice(-6)}` : "(none)"}`);
  console.log(`config: ${configPath()}`);
}

async function fetchSubmissionStatus(id, args = []) {
  return requestJson(`${apiUrl(args)}/api/submissions/${encodeURIComponent(id)}`, { headers: authHeaders() });
}

async function pollSubmissionStatus(id, args = []) {
  const intervalSeconds = numberFlag(args, "--poll-interval", 10);
  const timeoutSeconds = numberFlag(args, "--timeout", 0);
  const started = Date.now();
  let lastKey = "";
  while (true) {
    const response = await fetchSubmissionStatus(id, args);
    const key = `${response.status}:${response.rank_status}:${response.merge_commit_sha || ""}:${response.failure_code || ""}`;
    if (key !== lastKey) {
      printSubmissionStatus(response);
      lastKey = key;
    } else {
      console.log(`waiting: status=${response.status} rank_status=${response.rank_status}`);
    }
    if (isTerminalSubmission(response)) return response;
    if (timeoutSeconds > 0 && Date.now() - started >= timeoutSeconds * 1000) {
      throw new Error(`timed out waiting for ${id}`);
    }
    await sleep(intervalSeconds * 1000);
  }
}

async function submit(filePath, args) {
  filePath = filePath || defaultSubmissionPath();
  const metadata = readJson(filePath);
  const result = validatePackage(metadata, { trackId: getFlag(args, "--track", metadata.benchmark), metadataPath: filePath });
  printValidation(result);
  if (!result.ok) process.exit(1);
  await assertScoreImprovesLeaderboard(result.trackId, result.score, args);
  const note = readNoteOption(args, filePath, metadata);
  const payload = {
    track_id: result.trackId,
    metadata,
    model: getFlag(args, "--model", metadata.model || ""),
    note,
    source_url: getFlag(args, "--source-url", ""),
    ...readArchiveInfo(args, filePath, metadata)
  };
  const response = await requestJson(`${apiUrl(args)}/api/submissions`, {
    method: "POST",
    headers: authHeaders(),
    body: JSON.stringify(payload)
  });
  console.log(`submission_id: ${response.submission_id}`);
  console.log(`server_status: ${response.status}`);
  console.log(`rank_status: ${response.rank_status}`);
  if (hasFlag(args, "--watch")) {
    await pollSubmissionStatus(response.submission_id, args);
  }
}

async function status(id, args) {
  if (!id) usage(1);
  if (hasFlag(args, "--watch")) {
    await pollSubmissionStatus(id, args);
    return;
  }
  const response = await fetchSubmissionStatus(id, args);
  if (hasFlag(args, "--json")) {
    console.log(JSON.stringify(response, null, 2));
  } else {
    printSubmissionStatus(response);
  }
}

async function logs(id, args) {
  if (!id) usage(1);
  const response = await requestJson(`${apiUrl(args)}/api/submissions/${encodeURIComponent(id)}/logs`, { headers: authHeaders() });
  for (const entry of response.logs) console.log(`${entry.level.toUpperCase()} ${entry.code}: ${entry.message}`);
}

async function leaderboard(args) {
  const track = getFlag(args, "--track", repoManifest().name);
  const response = await requestJson(`${apiUrl(args)}/api/leaderboard?track_id=${encodeURIComponent(track)}`);
  if (!response.rows.length) {
    console.log("No accepted submissions yet.");
    return;
  }
  response.rows.forEach((row, index) => {
    const author = row.author_github_login ? `@${row.author_github_login}` : row.author_display_name;
    console.log(`${index + 1}. ${row.submission_name} ${row.score} ${row.submission_id} ${author}`);
  });
}

async function main() {
  const [command, first, ...rest] = process.argv.slice(2);
  if (!command || command === "--help" || command === "-h") usage(0);
  const args = [first, ...rest].filter(Boolean);
  if (isHelpFlag(first)) usage(0, command);

  if (command === "setup") return runManifestCommand("setupCommand");
  if (command === "run") return runManifestCommand("benchmarkCommand", args);
  if (command === "package") return packageSubmission(args);
  if (command === "validate") {
    const filePath = firstPositional(args) || defaultSubmissionPath();
    const result = validatePackage(readJson(filePath), { trackId: getFlag(args, "--track", null), metadataPath: filePath });
    printValidation(result);
    process.exit(result.ok ? 0 : 1);
  }
  if (command === "submit") return submit(firstPositional(args), args);
  if (command === "login") return login(first, rest);
  if (command === "config") return showConfig(args);
  if (command === "status") return status(first, rest);
  if (command === "logs") return logs(first, rest);
  if (command === "leaderboard") return leaderboard(args);
  usage(1);
}

main().catch((error) => {
  console.error(`error: ${error.message}`);
  process.exit(1);
});
