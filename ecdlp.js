#!/usr/bin/env node
"use strict";

const crypto = require("crypto");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");

const DEFAULT_API = "https://secp256k1.org";
const MAX_NOTE_BYTES = 10 * 1024;
const MAX_ARCHIVE_BYTES = 25 * 1024 * 1024;
const REQUIRED_SHOTS = 9024;
const SCORE_MODEL = "primitive-ccx-ccz-v1";
const REQUIRED_ARTIFACT = "ops.bin";

const TRACKS = {
  "point-double-secp256k1-v1": {
    gate: "fiat_shamir_point_double",
    editablePaths: ["src/point_double"],
    requiredChecks: ["classical correctness", "input preservation", "phase cleanliness", "ancilla cleanup"],
    defaultNoteFile: "src/point_double/memory/README.md"
  },
  "shor-ecdlp-5bit-v1": {
    gate: "fiat_shamir_shor_ecdlp_5bit_variable_q_oracle",
    editablePaths: ["src/shor_oracle"],
    requiredChecks: ["oracle correctness", "input preservation", "phase cleanliness", "ancilla cleanup"],
    defaultNoteFile: "src/shor_oracle/memory/README.md"
  },
  "shor-ecdlp-7bit-v1": {
    gate: "fiat_shamir_shor_ecdlp_7bit_variable_q_oracle",
    editablePaths: ["src/shor_oracle"],
    requiredChecks: ["oracle correctness", "input preservation", "phase cleanliness", "ancilla cleanup"],
    defaultNoteFile: "src/shor_oracle/memory/README.md"
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
  "--source-url",
  "--track"
]);

function usage(exitCode = 0) {
  console.log(`ecdlp baseline CLI

Usage:
  ./ecdlp.js setup
  ./ecdlp.js run [--note "short note"]
  ./ecdlp.js package --model MODEL [--note-file PATH] [--out dist]
  ./ecdlp.js validate [dist/submission-metadata.json]
  ./ecdlp.js submit [dist/submission-metadata.json] [--source-url URL]
  ./ecdlp.js login <api-key> [--api ${DEFAULT_API}]
  ./ecdlp.js config
  ./ecdlp.js status <submission-id>
  ./ecdlp.js logs <submission-id>
  ./ecdlp.js leaderboard [--track TRACK_ID]
`);
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
  return process.env.ECDLP_API_TOKEN || readConfig().api_token || "";
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

function utf8Bytes(text) {
  return Buffer.byteLength(text, "utf8");
}

function sameStringArray(left, right) {
  if (left.length !== right.length) return false;
  return left.every((value, index) => value === right[index]);
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
  for (const metricName of ["toffoli", "ccx", "ccz", "clifford", "qubits", "ops"]) {
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
  for (const metricName of ["toffoli", "ccx", "ccz", "clifford", "qubits", "ops"]) {
    if (!Number.isFinite(metrics[metricName]) || metrics[metricName] < 0) error("PACKAGE_METRIC", `metrics.${metricName} must be a non-negative finite number`);
  }
  const score = Math.round(Number(metrics.toffoli || 0)) * Math.round(Number(metrics.qubits || 0));
  if (!Number.isInteger(metadata.localScore) || metadata.localScore !== score) {
    error("PACKAGE_SCORE", `localScore must equal round(metrics.toffoli) * metrics.qubits (${score})`);
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
  return { archive_sha256: sha256File(archivePath), archive_size_bytes: fs.statSync(archivePath).size };
}

async function requestJson(url, options = {}) {
  const response = await fetch(url, {
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

async function submit(filePath, args) {
  filePath = filePath || defaultSubmissionPath();
  const metadata = readJson(filePath);
  const result = validatePackage(metadata, { trackId: getFlag(args, "--track", metadata.benchmark) });
  printValidation(result);
  if (!result.ok) process.exit(1);
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
}

async function status(id, args) {
  if (!id) usage(1);
  const response = await requestJson(`${apiUrl(args)}/api/submissions/${encodeURIComponent(id)}`, { headers: authHeaders() });
  console.log(JSON.stringify(response, null, 2));
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

  if (command === "setup") return runManifestCommand("setupCommand");
  if (command === "run") return runManifestCommand("benchmarkCommand", args);
  if (command === "package") return packageSubmission(args);
  if (command === "validate") {
    const filePath = firstPositional(args) || defaultSubmissionPath();
    const result = validatePackage(readJson(filePath), { trackId: getFlag(args, "--track", null) });
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
