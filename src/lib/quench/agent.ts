import { artifactFromInspection, FILE_TOO_LARGE, inspectFile, MAX_UPLOAD_BYTES } from "./inspect.ts";
import type { AgentDoctor, Artifact, NativeReport } from "./types.ts";

const PROXY = "/api/agent";
const DIRECT = "http://127.0.0.1:4783";

export const DEFAULT_SAMPLE_CONFIG = "samples/match-engine/quench.yaml";

export const AGENT_CONNECT_HINT =
  "Start the native agent on this machine with `npm run agent:serve` (or `quench-agent serve --bind 127.0.0.1:4783`). It listens on loopback and a unix socket. Binaries are not uploaded to the cloud. If Rust/cargo is not installed, Studio stays in Demo / Inspection mode.";

export const AGENT_UNAVAILABLE_HINT =
  "Local agent not connected. Sample cards are demo data. Uploaded files are inspection-only. Install Rust and run `npm run agent:serve` to run a real pipeline.";

export interface AgentStatus {
  connected: boolean;
  via: "proxy" | "direct" | null;
  version?: string;
  doctor?: AgentDoctor | null;
  error?: string;
  workspace?: string;
  sampleConfig?: string;
  sampleConfigPath?: string | null;
}

async function fetchJson(base: string, path: string, init?: RequestInit, timeoutMs = 4000): Promise<Response> {
  const ctrl = new AbortController();
  const t = setTimeout(() => ctrl.abort(), timeoutMs);
  try {
    return await fetch(`${base}${path}`, { ...init, signal: ctrl.signal });
  } finally {
    clearTimeout(t);
  }
}

function readSampleFields(body: {
  workspace?: string;
  sampleConfig?: string;
  sampleConfigPath?: string | null;
}): Pick<AgentStatus, "workspace" | "sampleConfig" | "sampleConfigPath"> {
  const sampleConfig =
    typeof body.sampleConfig === "string" && body.sampleConfig.trim()
      ? body.sampleConfig.trim()
      : DEFAULT_SAMPLE_CONFIG;
  return {
    workspace: typeof body.workspace === "string" ? body.workspace : undefined,
    sampleConfig,
    sampleConfigPath:
      typeof body.sampleConfigPath === "string" && body.sampleConfigPath.trim()
        ? body.sampleConfigPath.trim()
        : null,
  };
}

/** Relative workspace path used for the native sample. Never a hard-coded /workspace absolute path. */
export function nativeSampleConfig(status: AgentStatus | null | undefined): string {
  const fromAgent = status?.sampleConfig?.trim();
  if (fromAgent && !fromAgent.startsWith("/workspace/")) return fromAgent;
  const envPath = String(import.meta.env?.VITE_QUENCH_SAMPLE_CONFIG ?? "").trim();
  if (envPath && !envPath.startsWith("/workspace/")) return envPath;
  return DEFAULT_SAMPLE_CONFIG;
}

export async function probeAgent(): Promise<AgentStatus> {
  for (const via of ["proxy", "direct"] as const) {
    const base = via === "proxy" ? PROXY : DIRECT;
    try {
      const res = await fetchJson(base, "/v1/status", undefined, via === "direct" ? 800 : 2500);
      if (!res.ok) continue;
      const body = (await res.json()) as {
        ok?: boolean;
        version?: string;
        workspace?: string;
        sampleConfig?: string;
        sampleConfigPath?: string | null;
      };
      if (!body?.ok) continue;
      let doctor: AgentDoctor | null = null;
      try {
        const d = await fetchJson(base, "/v1/doctor", undefined, 4000);
        if (d.ok) doctor = (await d.json()) as AgentDoctor;
      } catch {
        doctor = null;
      }
      return {
        connected: true,
        via,
        version: body.version,
        doctor,
        ...readSampleFields(body),
      };
    } catch {
      continue;
    }
  }
  return { connected: false, via: null, error: AGENT_UNAVAILABLE_HINT, sampleConfig: DEFAULT_SAMPLE_CONFIG };
}

function baseFor(status: AgentStatus): string {
  return status.via === "direct" ? DIRECT : PROXY;
}

export async function agentInspect(
  file: File,
  status: AgentStatus,
): Promise<{ ok: true; artifact: Artifact } | { ok: false; error: string }> {
  if (file.size > MAX_UPLOAD_BYTES) return { ok: false, error: FILE_TOO_LARGE };
  if (!status.connected) {
    const local = await inspectFile(file);
    if (!local.ok) return local;
    return { ok: true, artifact: artifactFromInspection(local) };
  }
  try {
    const res = await fetch(`${baseFor(status)}/v1/inspect`, {
      method: "POST",
      headers: {
        "Content-Type": "application/octet-stream",
        "X-Filename": file.name,
      },
      body: file,
    });
    const body = (await res.json()) as {
      ok?: boolean;
      error?: string;
      kind?: "elf" | "docker" | "oci";
      name?: string;
      sizeBytes?: number;
      sha256?: string;
      elf?: Artifact["elf"];
      archive?: Artifact["archive"];
      recommendations?: string[];
    };
    if (!body.ok || !body.kind || !body.sha256) {
      return { ok: false, error: body.error ?? "Inspect failed" };
    }
    const artifact = artifactFromInspection({
      ok: true,
      kind: body.kind,
      name: body.name ?? file.name,
      sizeBytes: body.sizeBytes ?? file.size,
      sha256: body.sha256,
      elf: body.elf,
      archive: body.archive,
    });
    artifact.recommendations = body.recommendations;
    return { ok: true, artifact };
  } catch {
    return { ok: false, error: "Could not reach the local agent. " + AGENT_CONNECT_HINT };
  }
}

export interface AgentOptimizeResult {
  ok: boolean;
  runId: string;
  status: string;
  report: NativeReport & Record<string, unknown>;
  logs: string[];
  transformsApplied: { tool: string; status: string; detail: string }[];
  transformsFailed: { tool: string; status: string; detail: string }[];
  error?: string;
}

function failedOptimize(
  error: string,
  extra?: Partial<AgentOptimizeResult>,
): AgentOptimizeResult {
  return {
    ok: false,
    runId: extra?.runId ?? "",
    status: extra?.status ?? "failed",
    report: extra?.report ?? {},
    logs: extra?.logs ?? [],
    transformsApplied: extra?.transformsApplied ?? [],
    transformsFailed: extra?.transformsFailed ?? [],
    error,
  };
}

function tryParseJson(text: string): unknown {
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return undefined;
  }
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return value as Record<string, unknown>;
  }
  return null;
}

function asString(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value.trim() : undefined;
}

function httpErrorMessage(status: number, bodyError?: string): string {
  if (bodyError) {
    if (status === 403) return `Forbidden (403): ${bodyError}`;
    if (status === 404) return `Not found (404): ${bodyError}`;
    if (status === 502) return `Bad gateway (502): ${bodyError}`;
    return `Agent request failed (${status}): ${bodyError}`;
  }
  if (status === 403) return "The local agent rejected this request (403 Forbidden).";
  if (status === 404) return "The local agent endpoint was not found (404).";
  if (status === 502) return "The local agent proxy returned a bad gateway (502).";
  return `The local agent returned HTTP ${status}.`;
}

function fetchErrorMessage(err: unknown): string {
  const name =
    err && typeof err === "object" && "name" in err ? String((err as { name: string }).name) : "";
  const message = err instanceof Error ? err.message : "";
  if (name === "TimeoutError" || name === "AbortError" || /timeout|aborted/i.test(message)) {
    return "The local agent timed out while running optimize.";
  }
  return "Could not reach the local agent. " + AGENT_CONNECT_HINT;
}

function normalizeStatus(raw: unknown): string {
  return String(raw ?? "")
    .trim()
    .toLowerCase()
    .replace(/-/g, "_");
}

function isOptimizeShape(body: Record<string, unknown>): boolean {
  const report = asRecord(body.report);
  const status = asString(body.status) ?? (report ? asString(report.status) : undefined);
  const hasRunId = Boolean(asString(body.run_id) ?? asString(body.runId));
  const hasOk = typeof body.ok === "boolean" || (report && typeof report.ok === "boolean");
  return Boolean(status || report || hasRunId || hasOk);
}

function transformsOf(
  value: unknown,
): { tool: string; status: string; detail: string }[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    const rec = asRecord(item);
    if (!rec) return [];
    return [
      {
        tool: String(rec.tool ?? ""),
        status: String(rec.status ?? ""),
        detail: String(rec.detail ?? ""),
      },
    ];
  });
}

/** True when a native optimize response must not be shown as complete/successful. */
export function isNativeOptimizeFailure(result: AgentOptimizeResult): boolean {
  const status = normalizeStatus(result.status);
  if (result.ok === false) return true;
  if (status === "failed" || status === "verification_failed") return true;
  if (result.report.ok === false) return true;
  return false;
}

export async function agentOptimize(
  configPath: string,
  status: AgentStatus,
  timeoutMs = 10 * 60 * 1000,
): Promise<AgentOptimizeResult> {
  if (!status.connected) {
    return failedOptimize(AGENT_UNAVAILABLE_HINT);
  }

  let res: Response;
  try {
    res = await fetch(`${baseFor(status)}/v1/optimize`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ configPath }),
      signal: AbortSignal.timeout(timeoutMs),
    });
  } catch (err) {
    return failedOptimize(fetchErrorMessage(err));
  }

  let text = "";
  try {
    text = await res.text();
  } catch (err) {
    return failedOptimize(fetchErrorMessage(err));
  }

  const parsed = tryParseJson(text);
  const body = asRecord(parsed);
  const bodyError = body ? asString(body.error) : undefined;

  if (!res.ok) {
    return failedOptimize(httpErrorMessage(res.status, bodyError));
  }

  if (parsed === undefined) {
    return failedOptimize("The agent returned a non-JSON response.");
  }
  if (!body) {
    return failedOptimize("The agent response was missing the expected status/report shape.");
  }
  if (!isOptimizeShape(body)) {
    return failedOptimize("The agent response was missing the expected status/report shape.");
  }

  const reportRec = asRecord(body.report) ?? {};
  const report = {
    ...reportRec,
    project: reportRec.project ?? body.project,
    kind: reportRec.kind ?? body.kind,
    inputPath: reportRec.inputPath ?? body.input_path ?? body.inputPath,
    configPath: reportRec.configPath ?? body.config_path ?? body.configPath ?? configPath,
    ok: reportRec.ok ?? body.ok,
    error: reportRec.error ?? body.error,
  } as NativeReport & Record<string, unknown>;

  const runStatus = normalizeStatus(body.status ?? report.status) || "failed";
  const runId = asString(body.run_id) ?? asString(body.runId) ?? asString(report.runId) ?? "";
  const logs = Array.isArray(body.logs)
    ? body.logs.map((line) => String(line))
    : Array.isArray(report.logs)
      ? (report.logs as unknown[]).map((line) => String(line))
      : [];
  const appliedFinal = transformsOf(body.transforms_applied).length
    ? transformsOf(body.transforms_applied)
    : transformsOf(report.transformsApplied);
  const failedFinal = transformsOf(body.transforms_failed).length
    ? transformsOf(body.transforms_failed)
    : transformsOf(report.transformsFailed);

  const pipelineFailed =
    body.ok === false ||
    report.ok === false ||
    runStatus === "failed" ||
    runStatus === "verification_failed";

  const error =
    asString(body.error) ??
    (typeof report.error === "string" ? report.error : undefined) ??
    (typeof report.reason === "string" && pipelineFailed ? report.reason : undefined);

  return {
    ok: !pipelineFailed,
    runId,
    status: runStatus,
    report,
    logs,
    transformsApplied: appliedFinal,
    transformsFailed: failedFinal,
    error: pipelineFailed
      ? (error ?? "Native optimize failed.")
      : error,
  };
}

