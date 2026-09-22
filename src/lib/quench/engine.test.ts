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

test("createAgentRun maps a kept native candidate as complete, not modeled demo", () => {
  const report = {
    ok: true,
    status: "complete",
    project: "match-engine",
    kind: "elf",
    inputPath: "/tmp/match-engine/target/release/match-engine",
    configPath: "samples/match-engine/quench.yaml",
    buildCommand: "cargo build --release",
    testCommand: "./scripts/test.sh",
    profileCommand: "./scripts/workload.sh",
    profileMode: "instrument",
    profileReason: "LBR unavailable; using BOLT instrumentation on a copy",
    benchmarkedInstrumented: false,
    benchmarkedOriginal: true,
    benchmarkCommand: "./scripts/bench.sh",
    baselineSha256: "aa".repeat(32),
    candidateSha256: "bb".repeat(32),
    artifactSize: { baselineBytes: 4_700_184, candidateBytes: 404_352 },
    testResult: "Passed the supplied test suite",
    keptCandidate: true,
    minImprovementPercent: 1,
    maxRegressionPercent: 2,
    medianImprovementPercent: 1.7,
    medianMs: { baseline: 592.1, candidate: 582.0 },
    p95Ms: { baseline: 594.1, candidate: 586.5 },
    toolVersions: { strip: { status: "ok" }, "llvm-bolt": { status: "unavailable" } },
  };
  const artifact = artifactFromNativeReport(report, "samples/match-engine/quench.yaml");
  const run = createAgentRun({
    artifact,
    report,
    logs: ["[prove] kept candidate"],
    transformsApplied: [{ tool: "strip", status: "applied", detail: "strip --strip-unneeded" }],
    transformsFailed: [{ tool: "llvm-bolt", status: "Unavailable", detail: "not found on PATH" }],
    runId: "qnch_keep_1",
    agentStatus: "complete",
    ok: true,
  });
  assert.equal(run.status, "complete");
  assert.equal(run.mode, "agent");
  assert.equal(run.result.modeled, false);
  assert.equal(run.result.native?.keptCandidate, true);
  assert.equal(run.result.native?.buildCommand, "cargo build --release");
  assert.equal(run.result.native?.profileCommand, "./scripts/workload.sh");
  assert.equal(run.result.native?.profileMode, "instrument");
  assert.equal(run.result.native?.benchmarkedInstrumented, false);
  assert.equal(run.result.native?.benchmarkCommand, "./scripts/bench.sh");
  assert.equal(run.result.native?.minImprovementPercent, 1);
  assert.equal(run.result.native?.maxRegressionPercent, 2);
  assert.equal(run.result.native?.medianImprovementPercent, 1.7);
  assert.equal(run.artifact.source, "agent");
});

test("unavailable native tools stay native reports, not demo complete", () => {
  const report = {
    ok: true,
    status: "complete",
    project: "bare-elf",
    kind: "elf",
    inputPath: "/tmp/bare",
    buildCommand: "gcc -o app main.c",
    testCommand: "./test.sh",
    profileCommand: "./app",
    benchmarkCommand: "./bench.sh",
    keptCandidate: false,
    reason: "No candidate was produced. Missing tools are listed as Unavailable; success was not invented.",
    minImprovementPercent: 1,
    maxRegressionPercent: 2,
    testResult: "Passed the supplied test suite",
    toolVersions: { "llvm-bolt": { status: "unavailable" }, strip: { status: "unavailable" } },
  };
  const run = createAgentRun({
    artifact: artifactFromNativeReport(report),
    report,
    logs: ["[optimize] llvm-bolt: Unavailable"],
    transformsApplied: [],
    transformsFailed: [
      { tool: "llvm-bolt", status: "Unavailable", detail: "not found on PATH" },
      { tool: "strip", status: "Unavailable", detail: "not found on PATH" },
    ],
    runId: "qnch_unavail",
    agentStatus: "complete",
    ok: true,
  });
  assert.equal(run.mode, "agent");
  assert.equal(run.result.modeled, false);
  assert.equal(run.result.native?.keptCandidate, false);
  assert.equal(run.result.tools.some((t) => t.status === "unavailable"), true);
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


