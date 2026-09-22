import assert from "node:assert/strict";
import { test } from "node:test";
import {
  DEFAULT_SAMPLE_CONFIG,
  agentOptimize,
  isNativeOptimizeFailure,
  nativeSampleConfig,
  type AgentStatus,
} from "./agent.ts";
import { artifactFromNativeReport, createAgentRun, nativeRunStatus, runStatusLabel } from "./engine.ts";

const connected: AgentStatus = { connected: true, via: "proxy" };

function jsonResponse(status: number, body: unknown, contentType = "application/json"): Response {
  const text = typeof body === "string" ? body : JSON.stringify(body);
  return new Response(text, { status, headers: { "content-type": contentType } });
}

async function withFetch<T>(impl: typeof fetch, fn: () => Promise<T>): Promise<T> {
  const prev = globalThis.fetch;
  globalThis.fetch = impl;
  try {
    return await fn();
  } finally {
    globalThis.fetch = prev;
  }
}

test("native sample config is relative, not a hard-coded /workspace path", () => {
  assert.equal(DEFAULT_SAMPLE_CONFIG, "samples/match-engine/quench.yaml");
  assert.ok(!DEFAULT_SAMPLE_CONFIG.startsWith("/workspace"));
  assert.equal(nativeSampleConfig(null), DEFAULT_SAMPLE_CONFIG);
  const sample: AgentStatus = {
    connected: true,
    via: "proxy",
    sampleConfig: "samples/match-engine/quench.yaml",
    sampleConfigPath: "/tmp/project/samples/match-engine/quench.yaml",
  };
  assert.equal(nativeSampleConfig(sample), "samples/match-engine/quench.yaml");
  const hardcoded: AgentStatus = {
    connected: true,
    via: "proxy",
    sampleConfig: "/workspace/samples/match-engine/quench.yaml",
  };
  assert.equal(nativeSampleConfig(hardcoded), DEFAULT_SAMPLE_CONFIG);
});

test("custom QUENCH_SAMPLE_CONFIG is used as native sample identity", () => {
  const custom: AgentStatus = {
    connected: true,
    via: "proxy",
    sampleConfig: "samples/custom-acre/quench.yaml",
  };
  assert.equal(nativeSampleConfig(custom), "samples/custom-acre/quench.yaml");
});

test("agentOptimize treats runId + status=failed + ok=false as a failed run", async () => {
  const payload = {
    runId: "qnch_failed_1",
    status: "failed",
    ok: false,
    report: {
      ok: false,
      error: "baseline build failed",
      project: "custom-acre",
      kind: "elf",
      inputPath: "/tmp/custom-acre/target/release/custom-acre",
      artifactSize: { baselineBytes: 4096 },
      baselineSha256: "abc123",
    },
  };
  const result = await withFetch(async () => jsonResponse(200, payload), () =>
    agentOptimize("samples/custom-acre/quench.yaml", connected),
  );
  assert.equal(result.ok, false);
  assert.equal(result.runId, "qnch_failed_1");
  assert.equal(result.status, "failed");
  assert.equal(result.report.project, "custom-acre");
  assert.match(result.error ?? "", /baseline build failed/);
  assert.equal(isNativeOptimizeFailure(result), true);

  const artifact = artifactFromNativeReport(result.report, "samples/custom-acre/quench.yaml");
  assert.equal(artifact.name, "custom-acre");
  assert.notEqual(artifact.name, "match-engine");
  assert.equal(artifact.kind, "elf");
  assert.equal(artifact.sizeBytes, 4096);
  assert.equal(artifact.sha256, "abc123");
  assert.match(artifact.tag, /custom-acre/);

  const run = createAgentRun({
    artifact,
    report: result.report,
    logs: result.logs,
    transformsApplied: result.transformsApplied,
    transformsFailed: result.transformsFailed,
    runId: result.runId,
    agentStatus: result.status,
    ok: result.ok,
  });
  assert.equal(run.status, "failed");
  assert.equal(run.artifact.name, "custom-acre");
  assert.equal(runStatusLabel(run), "Failed");
  assert.notEqual(runStatusLabel(run), "Native report");
  assert.notEqual(run.status, "complete");
});

test("agentOptimize surfaces HTTP 403", async () => {
  const result = await withFetch(
    async () => jsonResponse(403, { ok: false, error: "origin not allowed" }),
    () => agentOptimize("samples/match-engine/quench.yaml", connected),
  );
  assert.equal(result.ok, false);
  assert.equal(result.runId, "");
  assert.match(result.error ?? "", /403|Forbidden|origin/i);
});

test("agentOptimize surfaces HTTP 404", async () => {
  const result = await withFetch(
    async () => jsonResponse(404, { ok: false, error: "not found" }),
    () => agentOptimize("samples/match-engine/quench.yaml", connected),
  );
  assert.equal(result.ok, false);
  assert.match(result.error ?? "", /404|Not found/i);
});

test("agentOptimize surfaces HTTP 502", async () => {
  const result = await withFetch(
    async () => new Response("Bad Gateway", { status: 502, headers: { "content-type": "text/plain" } }),
    () => agentOptimize("samples/match-engine/quench.yaml", connected),
  );
  assert.equal(result.ok, false);
  assert.match(result.error ?? "", /502|Bad gateway/i);
});

test("agentOptimize rejects malformed JSON", async () => {
  const result = await withFetch(
    async () => new Response("<html>nope</html>", { status: 200, headers: { "content-type": "text/html" } }),
    () => agentOptimize("samples/match-engine/quench.yaml", connected),
  );
  assert.equal(result.ok, false);
  assert.match(result.error ?? "", /non-JSON|not valid JSON|JSON/i);
});

test("agentOptimize rejects missing status/report shape", async () => {
  const result = await withFetch(
    async () => jsonResponse(200, { hello: "world" }),
    () => agentOptimize("samples/match-engine/quench.yaml", connected),
  );
  assert.equal(result.ok, false);
  assert.match(result.error ?? "", /status\/report|shape/i);
});

test("agentOptimize maps connection failure", async () => {
  const result = await withFetch(
    async () => {
      throw new TypeError("fetch failed");
    },
    () => agentOptimize("samples/match-engine/quench.yaml", connected),
  );
  assert.equal(result.ok, false);
  assert.match(result.error ?? "", /Could not reach|fetch failed|local agent/i);
});

test("agentOptimize maps timeout", async () => {
  const result = await withFetch(async () => {
    const err = new Error("The operation was aborted due to timeout");
    err.name = "TimeoutError";
    throw err;
  }, () => agentOptimize("samples/match-engine/quench.yaml", connected, 20));
  assert.equal(result.ok, false);
  assert.match(result.error ?? "", /timed out/i);
});

test("nativeRunStatus never promotes a failed ok=false run to complete", () => {
  assert.equal(
    nativeRunStatus({
      agentStatus: "failed",
      ok: false,
      report: { ok: false, error: "baseline build failed", keptCandidate: true },
    }),
    "failed",
  );
});
