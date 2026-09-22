import assert from "node:assert/strict";
import { test } from "node:test";
import { artifactFromNativeReport, createAgentRun, nativeRunStatus, runStatusLabel } from "./engine.ts";

test("createAgentRun stores a failed native response without marking it complete", () => {
  const report = {
    ok: false,
    status: "failed",
    error: "baseline build failed",
    project: "quiet-acre",
    kind: "elf",
    inputPath: "/opt/quiet-acre/target/release/quiet-acre",
    configPath: "samples/quiet-acre/quench.yaml",
    artifactSize: { baselineBytes: 8192 },
    baselineSha256: "deadbeef",
    keptCandidate: false,
  };
  const artifact = artifactFromNativeReport(report, "samples/quiet-acre/quench.yaml");
  const run = createAgentRun({
    artifact,
    report,
    logs: ["[rebuild] baseline build failed"],
    transformsApplied: [],
    transformsFailed: [],
    runId: "qnch_fail_diag",
    agentStatus: "failed",
    ok: false,
  });
  assert.equal(run.id, "qnch_fail_diag");
  assert.equal(run.status, "failed");
  assert.equal(run.mode, "agent");
  assert.equal(run.artifact.name, "quiet-acre");
  assert.equal(run.artifact.kind, "elf");
  assert.equal(run.artifact.sizeBytes, 8192);
  assert.equal(run.artifact.sha256, "deadbeef");
  assert.equal(run.result.native?.ok, false);
  assert.equal(run.result.native?.error, "baseline build failed");
  assert.equal(runStatusLabel(run), "Failed");
  assert.notEqual(run.status, "complete");
});

test("artifactFromNativeReport uses report identity instead of match-engine", () => {
  const artifact = artifactFromNativeReport(
    {
      project: "drum-core",
      kind: "elf",
      inputPath: "/src/drum-core/target/release/drum-core",
      artifactSize: { baselineBytes: 1024 },
      baselineSha256: "fff",
    },
    "samples/drum-core/quench.yaml",
  );
  assert.equal(artifact.name, "drum-core");
  assert.notEqual(artifact.name, "match-engine");
  assert.equal(artifact.tag.includes("match-engine"), false);
  assert.equal(artifact.sizeBytes, 1024);
});

test("nativeRunStatus maps verification_failed separately from complete", () => {
  assert.equal(
    nativeRunStatus({
      agentStatus: "verification_failed",
      ok: false,
      report: { ok: false, testResult: "Candidate discarded — supplied test suite failed" },
    }),
    "verification_failed",
  );
  assert.equal(
    nativeRunStatus({
      agentStatus: "complete",
      ok: true,
      report: { ok: true, keptCandidate: false },
    }),
    "complete",
  );
});
