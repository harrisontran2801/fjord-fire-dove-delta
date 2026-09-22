import { artifactFromInspection, FILE_TOO_LARGE, inspectFile, MAX_UPLOAD_BYTES } from "./inspect";
import type { AgentDoctor, Artifact, NativeReport } from "./types";

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

export async function agentOptimize(configPath: string, status: AgentStatus): Promise<AgentOptimizeResult> {
  if (!status.connected) {
    return {
      ok: false,
      runId: "",
      status: "failed",
      report: {},
      logs: [],
      transformsApplied: [],
      transformsFailed: [],
      error: AGENT_UNAVAILABLE_HINT,
    };
  }
  const res = await fetch(`${baseFor(status)}/v1/optimize`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ configPath }),
    signal: AbortSignal.timeout(10 * 60 * 1000),
  });
  const body = (await res.json()) as {
    run_id?: string;
    runId?: string;
    status?: string;
    report?: NativeReport & Record<string, unknown>;
    logs?: string[];
    transforms_applied?: { tool: string; status: string; detail: string }[];
    transforms_failed?: { tool: string; status: string; detail: string }[];
    ok?: boolean;
    error?: string;
  };
  const report = body.report ?? {};
  return {
    ok: Boolean(report.ok ?? body.status === "complete"),
    runId: body.run_id ?? body.runId ?? "",
    status: body.status ?? "failed",
    report,
    logs: body.logs ?? [],
    transformsApplied: body.transforms_applied ?? report.transformsApplied ?? [],
    transformsFailed: body.transforms_failed ?? report.transformsFailed ?? [],
    error: body.error ?? (typeof report.error === "string" ? report.error : undefined),
  };
}
