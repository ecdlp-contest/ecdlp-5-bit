"use strict";

const assert = require("node:assert/strict");
const { spawn } = require("node:child_process");
const http = require("node:http");
const path = require("node:path");
const test = require("node:test");

const ROOT = path.resolve(__dirname, "..");

function runCli(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(process.execPath, [path.join(ROOT, "ecdlp.js"), ...args], {
      cwd: ROOT,
      stdio: ["ignore", "pipe", "pipe"]
    });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => { stdout += chunk; });
    child.stderr.on("data", (chunk) => { stderr += chunk; });
    child.on("error", reject);
    child.on("close", (code) => resolve({ code, stdout, stderr }));
  });
}

async function verifyJsonResponse(payload) {
  let requestUrl = null;
  const server = http.createServer((request, response) => {
    requestUrl = request.url;
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify(payload));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();

  try {
    const result = await runCli([
      "leaderboard",
      "--track",
      "shor-ecdlp-5bit",
      "--api",
      `http://127.0.0.1:${port}`,
      "--json"
    ]);
    assert.equal(result.code, 0, result.stderr);
    assert.equal(result.stderr, "");
    assert.deepEqual(JSON.parse(result.stdout), payload);
    assert.equal(requestUrl, "/api/leaderboard?track_id=shor-ecdlp-5bit");
  } finally {
    await new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve()));
  }
}

test("leaderboard --json prints the complete API response", async () => {
  await verifyJsonResponse({
    rows: [{
      submission_id: "sub-1",
      submission_name: "candidate",
      author_github_login: "solver",
      score: 12.5,
      metrics: { qubits: 20 }
    }]
  });
  await verifyJsonResponse({ rows: [] });
});
