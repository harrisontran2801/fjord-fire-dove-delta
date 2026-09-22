export type ArtifactKind = "docker" | "elf" | "oci";
export type ParetoPref = "size" | "balanced" | "speed";
export type DimensionId = "runtime" | "artifact" | "resource" | "infra";
export type RunMode = "demo" | "inspect" | "agent";
export type RunStatus =
  | "running"
  | "complete"
  | "failed"
  | "verification_failed";
export type UiAgentState =
  | "demo"
  | "disconnected"
  | "connected"
  | "inspecting"
  | "optimizing"
  | "verification_failed"
  | "complete";
export type StageId =
  | "analyze"
  | "headroom"
  | "profile"
  | "optimize"
  | "rebuild"
  | "verify"
  | "benchmark"
  | "prove";

export interface ElfFacts {
  classBits: 32 | 64;
  endian: "little" | "big";
  machine: string;
  machineCode: number;
}

export interface ArchiveFacts {
  format: "docker-save" | "oci";
  entryCount: number;
  names: string[];
  repoTags?: string[];
  architecture?: string;
  layerCount?: number;
}

export interface Artifact {
  id: string;
  name: string;
  subtitle: string;
  kind: ArtifactKind;
  language: string;
  tag: string;
  customer: string;
  sizeBytes: number;
  sampleId?: string;
  source: "sample" | "upload" | "agent";
  sha256?: string;
  elf?: ElfFacts;
  archive?: ArchiveFacts;
  recommendations?: string[];
}

export interface Metrics {
  artifactBytes: number;
  layers: number;
  rssMb: number;
  cpuCores: number;
  p99Ms: number;
  throughputRps: number;
  coldStartMs: number;
  vulns: number;
  k8sCpu: string;
  k8sMem: string;
  sci: number;
  monthlyUsd: number;
}

export interface Headroom {
  id: DimensionId;
  label: string;
  before: number;
  after: number;
  sol: number;
  note: string;
}

export interface Layer {
  name: string;
  beforeMb: number;
  afterMb: number;
  action: string;
}

export interface TestCase {
  name: string;
  detail: string;
  durationMs: number;
  status: "pass" | "skip" | "warn" | "fail";
}

export interface ToolPass {
  tool: string;
  action: string;
  dimension: DimensionId;
  status?: "applied" | "failed" | "unavailable" | "skipped";
}

export interface StageDef {
  id: StageId;
  label: string;
  tool: string;
  duration: number;
}

export interface StageLog {
  id: StageId;
  lines: string[];
}

export interface CommandRecord {
  command: string;
  cwd?: string;
  exit_code: number;
  stdout?: string;
  stderr?: string;
  duration_ms: number;
  artifact_path?: string;
  sha256?: string;
  tool_version?: string;
  timed_out?: boolean;
}

export interface NativeReport {
  inputPath?: string;
  buildCommand?: string;
  testCommand?: string;
  benchmarkCommand?: string;
  toolVersions?: Record<string, { status?: string; version?: string; detail?: string; path?: string }>;
  baselineSha256?: string;
  candidateSha256?: string | null;
  artifactSize?: { baselineBytes?: number; candidateBytes?: number | null };
  testResult?: string | null;
  benchmarkRepetitions?: number;
  medianMs?: { baseline?: number; candidate?: number | null };
  p95Ms?: { baseline?: number; candidate?: number | null };
  transformsApplied?: { tool: string; status: string; detail: string }[];
  transformsFailed?: { tool: string; status: string; detail: string }[];
  reproducibleCommand?: string;
  keptCandidate?: boolean;
  reason?: string;
  runtime?: unknown;
  commands?: CommandRecord[];
  disclaimer?: string;
  minImprovementPercent?: number;
  maxRegressionPercent?: number;
  medianImprovementPercent?: number;
  error?: string;
  ok?: boolean;
  logs?: string[];
}

export interface RunResult {
  before: Metrics;
  after: Record<ParetoPref, Metrics>;
  headroom: Headroom[];
  layers: Layer[];
  tests: TestCase[];
  tools: ToolPass[];
  k8sBefore: string;
  k8sAfter: string;
  logs: StageLog[];
  certId: string;
  summary: string;
  modeled: boolean;
  native?: NativeReport;
}

export interface Run {
  id: string;
  createdAt: number;
  startedAt: number;
  artifact: Artifact;
  preference: ParetoPref;
  status: RunStatus;
  result: RunResult;
  mode: RunMode;
}

export interface Competitor {
  name: string;
  org: string;
  year: string;
  focus: string;
  localCloud: string;
  auto: string;
  dimensions: DimensionId[];
  verify: boolean;
  benchmark: boolean;
  cert: boolean;
}

export interface AgentDoctorCheck {
  id: string;
  name: string;
  status: "ok" | "warn" | "unavailable";
  detail: string;
  version?: string;
  path?: string;
}

export interface AgentDoctor {
  ok: boolean;
  supported_platform: boolean;
  platform: string;
  arch: string;
  work_dir: string;
  checks: AgentDoctorCheck[];
}
