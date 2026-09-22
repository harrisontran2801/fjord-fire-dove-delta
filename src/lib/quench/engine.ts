import { AGENT_PIPELINE, INSPECT_PIPELINE, PIPELINE, SAMPLES, TEMPLATES } from "./data";
import { certIdFrom, formatBytes, hashString } from "./format";
import type {
  Artifact,
  Metrics,
  NativeReport,
  ParetoPref,
  Run,
  RunMode,
  RunResult,
  RunStatus,
  StageDef,
  StageId,
} from "./types";

export { artifactFromFile } from "./inspect";

const EMPTY_METRICS: Metrics = {
  artifactBytes: 0,
  layers: 0,
  rssMb: 0,
  cpuCores: 0,
  p99Ms: 0,
  throughputRps: 0,
  coldStartMs: 0,
  vulns: 0,
  k8sCpu: "—",
  k8sMem: "—",
  sci: 0,
  monthlyUsd: 0,
};

function cloneMetrics(m: Metrics): Metrics {
  return { ...m };
}

export function runModeFor(artifact: Artifact): RunMode {
  if (artifact.source === "agent") return "agent";
  if (artifact.source === "upload") return "inspect";
  return "demo";
}

export function stagesFor(mode: RunMode | undefined): StageDef[] {
  if (mode === "inspect") return INSPECT_PIPELINE;
  if (mode === "agent") return AGENT_PIPELINE;
  return PIPELINE;
}

function kindLabel(kind: Artifact["kind"]): string {
  if (kind === "elf") return "ELF binary";
  if (kind === "oci") return "OCI archive";
  return "Docker save archive";
}

function buildInspectionResult(artifact: Artifact): RunResult {
  const certId = certIdFrom(artifact.sha256 ?? `${artifact.id}:${artifact.name}`);
  const sha = artifact.sha256 ?? "unavailable";
  const identify = [
    `quench inspect ${artifact.name}`,
    `identified  ${kindLabel(artifact.kind)}`,
    `sha256      ${sha}`,
    `size        ${formatBytes(artifact.sizeBytes)}  (${artifact.sizeBytes} bytes)`,
  ];
  if (artifact.elf) {
    identify.push(
      `elf         ELF${artifact.elf.classBits}  ${artifact.elf.endian}-endian  ${artifact.elf.machine}`,
    );
  }
  if (artifact.archive) {
    identify.push(`entries     ${artifact.archive.entryCount}`);
    if (artifact.archive.repoTags?.length) identify.push(`tags        ${artifact.archive.repoTags.join(", ")}`);
    if (artifact.archive.architecture) identify.push(`arch        ${artifact.archive.architecture}`);
    if (artifact.archive.layerCount) identify.push(`layers      ${artifact.archive.layerCount}`);
  }
  for (const rec of artifact.recommendations ?? []) identify.push(`note       ${rec}`);

  const listed = artifact.archive?.names?.slice(0, 8) ?? [];

  return {
    before: { ...EMPTY_METRICS, artifactBytes: artifact.sizeBytes },
    after: {
      size: { ...EMPTY_METRICS, artifactBytes: artifact.sizeBytes },
      balanced: { ...EMPTY_METRICS, artifactBytes: artifact.sizeBytes },
      speed: { ...EMPTY_METRICS, artifactBytes: artifact.sizeBytes },
    },
    headroom: [],
    layers: [],
    tests: [],
    tools: [
      {
        tool: "Magic-byte identifier",
        action: `Classified as ${kindLabel(artifact.kind)} from file contents`,
        dimension: "artifact",
      },
      { tool: "SHA-256", action: sha, dimension: "artifact" },
    ],
    k8sBefore: "",
    k8sAfter: "",
    logs: [
      { id: "analyze", lines: identify },
      {
        id: "headroom",
        lines: [
          "skip  Roofline / SOL gap is not measured without the native agent pipeline",
          "skip  No modeled headroom scores are shown for uploaded files",
        ],
      },
      {
        id: "profile",
        lines: [
          "skip  perf LBR and eBPF were not executed",
          listed.length ? `contents   ${listed.join(" · ")}` : "contents   headers parsed from the file only",
        ],
      },
      {
        id: "optimize",
        lines: [
          "skip  LLVM BOLT was not executed",
          "skip  strip / UPX / Zstd were not executed",
          "note  Connect the local agent and provide quench.yaml to run the native pipeline",
        ],
      },
      {
        id: "rebuild",
        lines: ["skip  No image or binary was rewritten"],
      },
      {
        id: "verify",
        lines: ["skip  No project test suite was run", "skip  Supplied test suite was not executed"],
      },
      {
        id: "benchmark",
        lines: ["skip  No before/after harness was executed"],
      },
      {
        id: "prove",
        lines: [
          `report   ${certId}`,
          "note     Inspection complete — verified file identity only",
          "note     This is not an optimization report and not a certificate.",
        ],
      },
    ],
    certId,
    summary: `Inspected ${artifact.name}: ${kindLabel(artifact.kind)}, ${formatBytes(artifact.sizeBytes)}, SHA-256 ${sha}. Inspection only — no optimizer tools were executed.`,
    modeled: false,
  };
}

export function buildResult(artifact: Artifact): RunResult {
  if (artifact.source !== "sample") return buildInspectionResult(artifact);

  const templateId = artifact.sampleId ?? "orders-api";
  const t = TEMPLATES[templateId] ?? TEMPLATES["orders-api"];
  const seed = hashString(`${artifact.name}:${artifact.sizeBytes}:${artifact.id}`);

  const after = {
    size: cloneMetrics(t.after.size),
    balanced: cloneMetrics(t.after.balanced),
    speed: cloneMetrics(t.after.speed),
  };

  const certId = certIdFrom(`${artifact.id}:${seed}`);
  const logs = t.logs.map((stage) => ({
    id: stage.id,
    lines: stage.lines.map((line) =>
      line
        .replace("QC-PLACEHOLDER", certId)
        .replaceAll(SAMPLES.find((s) => s.id === templateId)?.tag ?? "", artifact.tag)
        .replace("FUNCTIONAL EQUIVALENCE PASS", "SAMPLE VERIFICATION PASSED"),
    ),
  }));

  return {
    before: cloneMetrics(t.before),
    after,
    headroom: t.headroom.map((h) => ({ ...h })),
    layers: t.layers.map((l) => ({ ...l })),
    tests: t.tests.map((x) => ({ ...x })),
    tools: t.tools.map((x) => ({ ...x })),
    k8sBefore: t.k8sBefore,
    k8sAfter: t.k8sAfter.replace("QC-PLACEHOLDER", certId),
    logs,
    certId,
    summary: t.summary,
    modeled: true,
  };
}

export function newRunId(): string {
  return `qnch_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

export function createRun(artifact: Artifact, preference: ParetoPref): Run {
  const now = Date.now();
  const mode = runModeFor(artifact);
  return {
    id: newRunId(),
    createdAt: now,
    startedAt: now,
    artifact,
    preference,
    status: "running",
    result: buildResult(artifact),
    mode,
  };
}

export function createAgentRun(args: {
  artifact: Artifact;
  report: NativeReport & Record<string, unknown>;
  logs: string[];
  transformsApplied: { tool: string; status: string; detail: string }[];
  transformsFailed: { tool: string; status: string; detail: string }[];
  runId?: string;
  agentStatus?: string;
}): Run {
  const now = Date.now();
  const id = args.runId && args.runId.length > 0 ? args.runId : newRunId();
  const kept = Boolean(args.report.keptCandidate);
  const sizeBefore = args.report.artifactSize?.baselineBytes ?? args.artifact.sizeBytes;
  const sizeAfter = kept
    ? (args.report.artifactSize?.candidateBytes ?? sizeBefore)
    : sizeBefore;
  const medBefore = args.report.medianMs?.baseline ?? 0;
  const medAfter = kept ? (args.report.medianMs?.candidate ?? medBefore) : medBefore;
  const p95Before = args.report.p95Ms?.baseline ?? 0;
  const p95After = kept ? (args.report.p95Ms?.candidate ?? p95Before) : p95Before;
  const metricsBefore: Metrics = {
    ...EMPTY_METRICS,
    artifactBytes: Number(sizeBefore) || 0,
    p99Ms: Number(p95Before) || 0,
  };
  const metricsAfter: Metrics = {
    ...EMPTY_METRICS,
    artifactBytes: Number(sizeAfter) || 0,
    p99Ms: Number(p95After) || 0,
  };
  const stageLines: Record<string, string[]> = {
    analyze: [],
    rebuild: [],
    verify: [],
    benchmark: [],
    profile: [],
    optimize: [],
    headroom: [],
    prove: [],
  };
  for (const line of args.logs) {
    const m = line.match(/^\[([a-z]+)\]\s*(.*)$/i);
    const stage = (m?.[1] ?? "prove").toLowerCase();
    const rest = m?.[2] ?? line;
    const key = stage in stageLines ? stage : "prove";
    stageLines[key]?.push(rest);
  }
  const native: NativeReport = {
    inputPath: String(args.report.inputPath ?? args.artifact.name),
    buildCommand: args.report.buildCommand ? String(args.report.buildCommand) : undefined,
    testCommand: args.report.testCommand ? String(args.report.testCommand) : undefined,
    benchmarkCommand: args.report.benchmarkCommand ? String(args.report.benchmarkCommand) : undefined,
    toolVersions: args.report.toolVersions,
    baselineSha256: args.report.baselineSha256 ? String(args.report.baselineSha256) : undefined,
    candidateSha256: args.report.candidateSha256 == null ? null : String(args.report.candidateSha256),
    artifactSize: args.report.artifactSize,
    testResult: args.report.testResult ? String(args.report.testResult) : null,
    benchmarkRepetitions: args.report.benchmarkRepetitions,
    medianMs: args.report.medianMs,
    p95Ms: args.report.p95Ms,
    transformsApplied: args.transformsApplied,
    transformsFailed: args.transformsFailed,
    reproducibleCommand:
      args.report.reproducibleCommand ? String(args.report.reproducibleCommand) : "quench-agent optimize --config quench.yaml",
    keptCandidate: Boolean(args.report.keptCandidate),
    reason: args.report.reason ? String(args.report.reason) : undefined,
    runtime: args.report.runtime,
    commands: args.report.commands,
    disclaimer:
      args.report.disclaimer && !/certificate|certified/i.test(String(args.report.disclaimer))
        ? String(args.report.disclaimer)
        : "Local optimization report. Not an ISO certificate, third-party certificate, or certified result.",
    minImprovementPercent: args.report.minImprovementPercent,
    maxRegressionPercent: args.report.maxRegressionPercent,
    medianImprovementPercent: args.report.medianImprovementPercent,
    error: args.report.error ? String(args.report.error) : undefined,
    ok: args.report.ok,
  };
  const status: RunStatus =
    args.agentStatus === "verification_failed"
      ? "verification_failed"
      : args.agentStatus === "complete" && args.report.ok !== false
        ? "complete"
        : args.report.keptCandidate
          ? "complete"
          : args.agentStatus === "failed" || args.report.ok === false
            ? args.report.testResult?.toLowerCase().includes("fail")
              ? "verification_failed"
              : "failed"
            : "complete";

  const result: RunResult = {
    before: metricsBefore,
    after: { size: cloneMetrics(metricsAfter), balanced: cloneMetrics(metricsAfter), speed: cloneMetrics(metricsAfter) },
    headroom: [],
    layers: [],
    tests: native.testResult
      ? [
          {
            name: "supplied test suite",
            detail: native.testResult,
            durationMs: 0,
            status: native.testResult.startsWith("Passed") ? "pass" : "fail",
          },
        ]
      : [],
    tools: [
      ...args.transformsApplied.map((t) => ({
        tool: t.tool,
        action: t.detail,
        dimension: "artifact" as const,
        status: "applied" as const,
      })),
      ...args.transformsFailed.map((t) => ({
        tool: t.tool,
        action: t.detail,
        dimension: "artifact" as const,
        status: (t.status.toLowerCase() === "unavailable" ? "unavailable" : "failed") as "unavailable" | "failed",
      })),
    ],
    k8sBefore: "",
    k8sAfter: "",
    logs: (Object.keys(stageLines) as StageId[]).map((id) => ({
      id,
      lines: stageLines[id] ?? [],
    })),
    certId: id,
    summary:
      String(args.report.reason ?? args.report.error ?? native.testResult ?? "Local agent pipeline finished."),
    modeled: false,
    native,
  };

  return {
    id,
    createdAt: now,
    startedAt: now,
    artifact: { ...args.artifact, source: "agent" },
    preference: "balanced",
    status,
    result,
    mode: "agent",
  };
}

export function stageEndTimes(startedAt: number, mode?: RunMode): number[] {
  let t = startedAt;
  return stagesFor(mode).map((s) => {
    t += s.duration;
    return t;
  });
}

export function revealedStage(run: Run, now = Date.now()): number {
  const stages = stagesFor(run.mode);
  if (run.status !== "running") return stages.length;
  if (run.mode === "agent") return stages.length;
  const ends = stageEndTimes(run.startedAt, run.mode);
  for (let i = 0; i < ends.length; i++) {
    if (now < (ends[i] ?? 0)) return i;
  }
  return stages.length;
}

export function stageProgress(run: Run, now = Date.now()): { index: number; frac: number } {
  const stages = stagesFor(run.mode);
  const idx = revealedStage(run, now);
  if (idx >= stages.length) return { index: stages.length, frac: 1 };
  const start = idx === 0 ? run.startedAt : (stageEndTimes(run.startedAt, run.mode)[idx - 1] ?? run.startedAt);
  const end = stageEndTimes(run.startedAt, run.mode)[idx] ?? start + 1;
  const frac = Math.min(1, Math.max(0, (now - start) / Math.max(1, end - start)));
  return { index: idx, frac };
}

export function revealedLogs(run: Run, now = Date.now()): { stage: StageId; line: string }[] {
  const { index, frac } = stageProgress(run, now);
  const stages = stagesFor(run.mode);
  const out: { stage: StageId; line: string }[] = [];
  for (let i = 0; i < stages.length; i++) {
    const stage = stages[i];
    if (!stage) continue;
    const block = run.result.logs.find((l) => l.id === stage.id);
    if (!block) continue;
    if (i < index) {
      for (const line of block.lines) out.push({ stage: stage.id, line });
    } else if (i === index) {
      const n = Math.max(1, Math.ceil(block.lines.length * Math.max(frac, 0.12)));
      for (const line of block.lines.slice(0, n)) out.push({ stage: stage.id, line });
    }
  }
  return out;
}

export function activeMetrics(run: Run): Metrics {
  return run.result.after[run.preference];
}

export function pipelineCompleteMs(mode?: RunMode): number {
  return stagesFor(mode).reduce((a, s) => a + s.duration, 0);
}

export function isDemo(run: Run): boolean {
  return run.mode === "demo";
}

export function isAgentRun(run: Run): boolean {
  return run.mode === "agent";
}

export function isInspect(run: Run): boolean {
  return run.mode === "inspect";
}

export function runStatusLabel(run: Run): string {
  if (run.mode === "agent") {
    if (run.status === "verification_failed") return "Tests failed";
    if (run.status === "failed") return "Candidate not kept";
    if (run.status === "running") return "Optimizing";
    if (run.result.native?.keptCandidate) return "Native report";
    return "Baseline retained";
  }
  if (run.mode === "inspect") {
    return run.status === "complete" ? "Inspected (inspection only)" : "Inspecting";
  }
  return run.status === "complete" ? "Demo report" : "Running";
}

export function demoAndAgentMixed(runs: Run[]): boolean {
  return runs.some((r) => r.mode === "demo" && r.result.modeled === false && r.artifact.source === "agent");
}
