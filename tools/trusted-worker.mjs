#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const ROOT_DIR = path.resolve(new URL("..", import.meta.url).pathname);
const API_URL = (process.env.ECDLP_API_URL || "").replace(/\/$/, "");
const WORKER_TOKEN = process.env.ECDLP_TRUSTED_WORKER_TOKEN || "";
const parsedLimit = Number(process.env.ECDLP_WORKER_LIMIT || 1);
const LIMIT = Number.isFinite(parsedLimit) && parsedLimit > 0 ? parsedLimit : 1;
const DRY_RUN = process.env.ECDLP_WORKER_DRY_RUN === "1" || process.argv.includes("--dry-run");
const NOTE_TEXT = "trusted worker reproduction";
const WORKER_ENV = { CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR || "target" };

function readJson(filePath) { return JSON.parse(fs.readFileSync(path.resolve(filePath), "utf8")); }
function run(command, args, options = {}) {
  console.log("> " + [command, ...args].join(" "));
  const result = spawnSync(command, args, { cwd: options.cwd || ROOT_DIR, env: { ...process.env, ...WORKER_ENV, ...(options.env || {}) }, stdio: options.capture ? ["ignore", "pipe", "pipe"] : "inherit", encoding: "utf8", shell: false });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(command + " failed with exit code " + result.status + (result.stderr ? "\n" + result.stderr : ""));
  return result.stdout || "";
}
async function requestJson(url, options = {}) {
  const response = await fetch(url, { ...options, headers: { "content-type": "application/json", "x-ecdlp-worker-token": WORKER_TOKEN, ...(options.headers || {}) } });
  const text = await response.text();
  const body = text ? JSON.parse(text) : {};
  if (!response.ok) throw new Error(body.error || "HTTP " + response.status);
  return body;
}
function sha256(buffer) { return crypto.createHash("sha256").update(buffer).digest("hex"); }
function assertEqual(label, actual, expected) { if (expected !== null && expected !== undefined && expected !== "" && actual !== expected) throw new Error(label + " mismatch: reproduced=" + actual + " submitted=" + expected); }
async function downloadArchive(submission, targetPath) {
  if (!submission.archive_url) throw new Error("pending submission has no archive_url");
  const response = await fetch(new URL(submission.archive_url, API_URL).toString(), { headers: { "x-ecdlp-worker-token": WORKER_TOKEN } });
  if (!response.ok) throw new Error("archive download failed with HTTP " + response.status + ": " + await response.text().catch(() => ""));
  const buffer = Buffer.from(await response.arrayBuffer());
  assertEqual("archive_size_bytes", buffer.length, submission.archive_size_bytes);
  assertEqual("archive_sha256", sha256(buffer), submission.archive_sha256);
  fs.writeFileSync(targetPath, buffer);
}
function validateArchiveEntries(manifest, archivePath) {
  const listing = run("tar", ["-tzf", archivePath], { capture: true }).split(/\r?\n/).map((entry) => entry.trim()).filter(Boolean);
  const editablePaths = new Set((manifest.editablePaths || []).map((entry) => entry.replace(/\/+$/g, "") + "/"));
  if (listing.length === 0) throw new Error("submission archive is empty");
  for (const entry of listing) {
    if (entry.startsWith("/") || entry.split("/").includes("..")) throw new Error("unsafe archive entry: " + entry);
    if (![...editablePaths].some((editablePath) => entry === editablePath.slice(0, -1) || entry.startsWith(editablePath))) throw new Error("archive entry is outside editable paths: " + entry);
  }
}
function prepareScripts() { for (const script of ["ecdlp.js", "setup.sh", "benchmark.sh"]) { const filePath = path.join(ROOT_DIR, script); if (fs.existsSync(filePath)) fs.chmodSync(filePath, fs.statSync(filePath).mode | 0o755); } }
function resetGeneratedOutputs() { for (const filePath of ["ops.bin", "score.json", "results.tsv"]) fs.rmSync(path.join(ROOT_DIR, filePath), { force: true }); fs.rmSync(path.join(ROOT_DIR, "dist"), { recursive: true, force: true }); }
function compareMetadata(metadata, submission) {
  assertEqual("track_id", metadata.benchmark, submission.track_id); assertEqual("score_model", metadata.scoreModel, submission.score_model); assertEqual("artifact_binary_sha256", metadata.artifactSha256, submission.artifact_binary_sha256); assertEqual("score", metadata.localScore, submission.metrics?.score); assertEqual("metrics.logical_qubits", metadata.metrics?.qubits, submission.metrics?.logical_qubits); assertEqual("metrics.toffoli_count", metadata.metrics?.toffoli, submission.metrics?.toffoli_count); assertEqual("metrics.ccx_count", metadata.metrics?.ccx, submission.metrics?.ccx_count); assertEqual("metrics.ccz_count", metadata.metrics?.ccz, submission.metrics?.ccz_count); assertEqual("metrics.clifford_count", metadata.metrics?.clifford, submission.metrics?.clifford_count); assertEqual("metrics.ops_count", metadata.metrics?.ops, submission.metrics?.ops_count); assertEqual("metrics.artifact_binary_size_bytes", metadata.artifactBytes, submission.metrics?.artifact_binary_size_bytes);
}
function noteFileFor(manifest) { const editablePath = manifest.editablePaths?.[0]; const candidates = [editablePath ? path.join(ROOT_DIR, editablePath, "memory", "README.md") : null, editablePath ? path.join(ROOT_DIR, editablePath, "README.md") : null, path.join(ROOT_DIR, "README.md")].filter(Boolean); for (const candidate of candidates) if (fs.existsSync(candidate)) return candidate; const generated = path.join(ROOT_DIR, ".trusted-worker-note.md"); fs.writeFileSync(generated, NOTE_TEXT + "\n"); return generated; }
function hasStagedChanges() { const result = spawnSync("git", ["diff", "--cached", "--quiet"], { cwd: ROOT_DIR, stdio: "ignore", shell: false }); if (result.error) throw result.error; return result.status === 1; }
function currentCommit() { return run("git", ["rev-parse", "HEAD"], { capture: true }).trim(); }
function acceptCommitMessage(submission, metadata) { return ["Accept " + submission.track_id + " submission " + submission.submission_id, "", "Submission: " + submission.submission_id, "Track: " + submission.track_id, "Score: " + metadata.localScore, "Artifact: " + metadata.artifactSha256, "Model: " + (submission.submitted_model || ""), submission.author_github_login ? "Author: " + submission.author_github_login + " <" + submission.author_github_login + "@users.noreply.github.com>" : ""].filter(Boolean).join("\n"); }
function commitAndPush(submission, manifest, metadata) { for (const editablePath of manifest.editablePaths || []) run("git", ["add", editablePath]); if (!hasStagedChanges()) { console.log("No editable-path changes to commit; using current HEAD."); return currentCommit(); } run("git", ["config", "user.name", "ecdlp-trusted-worker"]); run("git", ["config", "user.email", "ecdlp-trusted-worker@users.noreply.github.com"]); run("git", ["commit", "-m", acceptCommitMessage(submission, metadata)]); run("git", ["push", "origin", "HEAD:main"]); return currentCommit(); }
async function processSubmission(submission, manifest) {
  console.log("\nProcessing " + submission.submission_id + " (" + submission.track_id + ")");
  const archivePath = path.join(ROOT_DIR, ".trusted-submission.tar.gz"); await downloadArchive(submission, archivePath); validateArchiveEntries(manifest, archivePath); resetGeneratedOutputs(); for (const editablePath of manifest.editablePaths || []) fs.rmSync(path.join(ROOT_DIR, editablePath), { recursive: true, force: true }); run("tar", ["-xzf", archivePath, "-C", ROOT_DIR]); prepareScripts(); run(process.execPath, [path.join(ROOT_DIR, "ecdlp.js"), "setup"]); run(process.execPath, [path.join(ROOT_DIR, "ecdlp.js"), "run", "--note", NOTE_TEXT]); run("pwsh", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", path.join(ROOT_DIR, "tools", "package-submission.ps1"), "-NoteFile", noteFileFor(manifest), "-Model", submission.submitted_model || "trusted-worker"]); run(process.execPath, [path.join(ROOT_DIR, "ecdlp.js"), "validate", path.join(ROOT_DIR, "dist", "submission-metadata.json")]); const metadata = readJson(path.join(ROOT_DIR, "dist", "submission-metadata.json")); compareMetadata(metadata, submission); if (DRY_RUN) { console.log("dry-run: not committing source or posting trusted-pass"); return; } const acceptedCommit = commitAndPush(submission, manifest, metadata); const response = await requestJson(API_URL + "/api/submissions/" + encodeURIComponent(submission.submission_id) + "/trusted-pass", { method: "POST", body: JSON.stringify({ status: "passed", report: { worker: process.env.GITHUB_WORKFLOW || "contest-repo-trusted-worker", run_id: process.env.GITHUB_RUN_ID || null, accepted_repository: process.env.GITHUB_REPOSITORY || null, accepted_commit_sha: acceptedCommit, score: metadata.localScore, artifact_binary_sha256: metadata.artifactSha256, archive_sha256: submission.archive_sha256 || null } }) }); console.log("accepted " + response.submission_id + ": " + response.status + "/" + response.rank_status);
}
async function main() { if (process.argv.includes("--help") || process.argv.includes("-h")) return; if (!API_URL) throw new Error("ECDLP_API_URL is required"); if (!WORKER_TOKEN) throw new Error("ECDLP_TRUSTED_WORKER_TOKEN is required"); const manifest = readJson(path.join(ROOT_DIR, "benchmark.json")); const pending = await requestJson(API_URL + "/api/trusted-worker/submissions/pending?track_id=" + encodeURIComponent(manifest.name) + "&limit=" + encodeURIComponent(String(LIMIT))); if (!pending.rows.length) { console.log("No pending trusted-worker submissions for " + manifest.name + "."); return; } const failures = []; for (const submission of pending.rows) { try { await processSubmission(submission, manifest); } catch (error) { failures.push({ submission_id: submission.submission_id, error: error.message }); console.error("failed " + submission.submission_id + ": " + error.message); } } if (failures.length > 0) { console.error(JSON.stringify({ failures }, null, 2)); process.exit(1); } }
main().catch((error) => { console.error("trusted worker error: " + error.message); process.exit(1); });
