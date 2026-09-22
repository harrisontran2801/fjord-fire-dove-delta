import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";
import { Box, Cpu, LoaderCircle, Trash2, Upload } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Shell } from "@/components/layout/shell";
import { AgentBanner } from "@/components/studio/agent-status";
import { DEMO_LABEL, INSPECTION_ONLY_LABEL, PREF_COPY, SAMPLE_BY_ID, SAMPLES } from "@/lib/quench/data";
import {
  AGENT_CONNECT_HINT,
  AGENT_UNAVAILABLE_HINT,
  nativeSampleConfig,
  agentInspect,
  agentOptimize,
  probeAgent,
  type AgentStatus,
} from "@/lib/quench/agent";
import { artifactFromFile } from "@/lib/quench/engine";
import { formatBytes, relativeTime } from "@/lib/quench/format";
import { useQuenchStore } from "@/lib/quench/store";
import type { Artifact, ParetoPref } from "@/lib/quench/types";
import { runStatusLabel } from "@/lib/quench/engine";
import { cn } from "@/lib/utils";

export const Route = createFileRoute("/studio/")({ component: StudioPage });

function StudioPage() {
  const navigate = useNavigate();
  const inputRef = useRef<HTMLInputElement>(null);
  const [drag, setDrag] = useState(false);
  const [reading, setReading] = useState(false);
  const [readingName, setReadingName] = useState("");
  const [uploadError, setUploadError] = useState<string | null>(null);
  const [agent, setAgent] = useState<AgentStatus | null>(null);
  const [nativeBusy, setNativeBusy] = useState(false);
  const [nativeError, setNativeError] = useState<string | null>(null);
  const preference = useQuenchStore((s) => s.preference);
  const setPreference = useQuenchStore((s) => s.setPreference);
  const startRun = useQuenchStore((s) => s.startRun);
  const startAgentRun = useQuenchStore((s) => s.startAgentRun);
  const runs = useQuenchStore((s) => s.runs);
  const hydrated = useQuenchStore((s) => s.hydrated);
  const setHydrated = useQuenchStore((s) => s.setHydrated);
  const removeRun = useQuenchStore((s) => s.removeRun);

  useEffect(() => {
    if (!hydrated) setHydrated();
  }, [hydrated, setHydrated]);

  useEffect(() => {
    let alive = true;
    const tick = () => {
      void probeAgent().then((s) => {
        if (alive) setAgent(s);
      });
    };
    tick();
    const id = window.setInterval(tick, 4000);
    return () => {
      alive = false;
      window.clearInterval(id);
    };
  }, []);

  function launch(artifact: Artifact) {
    const run = startRun(artifact, preference);
    void navigate({ to: "/studio/$runId", params: { runId: run.id } });
  }

  async function onFiles(files: FileList | null) {
    const file = files?.[0];
    if (!file || reading) return;
    setUploadError(null);
    setReading(true);
    setReadingName(file.name);
    try {
      let result: { ok: true; artifact: Artifact } | { ok: false; error: string };
      if (agent?.connected) {
        result = await agentInspect(file, agent);
        if (!result.ok) {
          result = await artifactFromFile(file);
        }
      } else {
        result = await artifactFromFile(file);
      }
      if (!result.ok) {
        setUploadError(result.error);
        return;
      }
      launch(result.artifact);
    } catch {
      setUploadError("Could not read this file. Check that it is readable and try again.");
    } finally {
      setReading(false);
      setReadingName("");
      if (inputRef.current) inputRef.current.value = "";
    }
  }

  async function runNativeSample() {
    if (!agent?.connected || nativeBusy) return;
    setNativeError(null);
    setNativeBusy(true);
    try {
      const result = await agentOptimize(nativeSampleConfig(agent), agent);
      if (!result.runId) {
        setNativeError(result.error ?? "Native optimize did not return a run.");
        return;
      }
      const sample = SAMPLE_BY_ID["match-engine"];
      if (!sample) {
        setNativeError("match-engine sample is missing from the studio.");
        return;
      }
      const size =
        Number(result.report.artifactSize?.baselineBytes) || sample.sizeBytes;
      const run = startAgentRun({
        artifact: { ...sample, sizeBytes: size, source: "agent" },
        report: result.report,
        logs: result.logs,
        transformsApplied: result.transformsApplied,
        transformsFailed: result.transformsFailed,
        runId: result.runId,
        agentStatus: result.status,
      });
      void navigate({ to: "/studio/$runId", params: { runId: run.id } });
    } catch (err) {
      setNativeError(
        err instanceof Error ? err.message : "Native optimize failed. " + AGENT_CONNECT_HINT,
      );
    } finally {
      setNativeBusy(false);
    }
  }

  const connected = Boolean(agent?.connected);

  return (
    <Shell wide>
      <main className="mx-auto max-w-7xl px-4 py-8 sm:px-6">
        <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
          <div>
            <p className="text-xs font-medium uppercase tracking-[0.16em] text-subtle">Studio</p>
            <h1 className="mt-2 font-display text-3xl font-semibold">Local optimization workbench</h1>
            <p className="mt-2 max-w-xl text-sm text-muted">
              Sample workloads use labeled demo data. Uploaded ELF and Docker/OCI archives are
              identified from file contents and stay inspection-only — they never start a modeled
              optimizer. Native optimize needs the local agent and a project config inside the
              workspace.
            </p>
          </div>
          <fieldset className="flex rounded-lg bg-surface p-1 shadow-[var(--shadow-border)]">
            <legend className="sr-only">Pareto preference</legend>
            {(Object.keys(PREF_COPY) as ParetoPref[]).map((key) => (
              <button
                key={key}
                type="button"
                onClick={() => setPreference(key)}
                className={cn(
                  "h-10 rounded-md px-3 text-sm transition-colors duration-150",
                  preference === key ? "bg-accent text-accent-fg" : "text-muted hover:text-fg",
                )}
              >
                {PREF_COPY[key].label}
              </button>
            ))}
          </fieldset>
        </div>
        <p className="mt-3 text-xs text-subtle">{PREF_COPY[preference].hint}</p>

        <div className="mt-6">
          <AgentBanner agent={agent} />
        </div>

        <div className="mt-4 rounded-xl bg-surface p-4 shadow-[var(--shadow-border)]">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <p className="text-sm font-medium">Native match-engine sample</p>
              <p className="mt-1 text-xs text-muted">
                Runs the real agent against{" "}
                <span className="font-mono">{nativeSampleConfig(agent)}</span> inside{" "}
                <span className="font-mono">{agent?.workspace ?? "QUENCH_WORKSPACE"}</span>.
                Override with <span className="font-mono">QUENCH_SAMPLE_CONFIG</span>. Uploaded files
                are never treated as this project.
              </p>
            </div>
            {connected ? (
              <Button type="button" onClick={() => void runNativeSample()} disabled={nativeBusy}>
                {nativeBusy ? (
                  <>
                    <LoaderCircle className="size-4 animate-spin" />
                    Running native sample…
                  </>
                ) : (
                  "Run native sample"
                )}
              </Button>
            ) : (
              <p className="max-w-sm text-xs text-muted">
                Agent disconnected — studio stays in Demo / Inspection mode. {AGENT_UNAVAILABLE_HINT}
              </p>
            )}
          </div>
          {nativeError ? (
            <p role="alert" className="mt-3 text-sm text-danger">
              {nativeError}
            </p>
          ) : null}
        </div>

        <label
          onDragOver={(e) => {
            e.preventDefault();
            if (!reading) setDrag(true);
          }}
          onDragLeave={() => setDrag(false)}
          onDrop={(e) => {
            e.preventDefault();
            setDrag(false);
            if (!reading) void onFiles(e.dataTransfer.files);
          }}
          className={cn(
            "mt-8 flex min-h-40 w-full cursor-pointer flex-col items-center justify-center rounded-2xl bg-surface px-6 text-center shadow-[var(--shadow-border)] transition-[box-shadow,background-color] duration-200",
            drag && "bg-surface-2 shadow-[var(--shadow-border-hover)]",
            reading && "pointer-events-none opacity-80",
          )}
        >
          {reading ? (
            <>
              <LoaderCircle className="size-6 animate-spin text-accent" />
              <p className="mt-3 text-sm font-medium">Reading file…</p>
              <p className="mt-1 max-w-full truncate text-xs text-muted">{readingName}</p>
            </>
          ) : (
            <>
              <Upload className="size-6 text-accent" />
              <p className="mt-3 text-sm font-medium">Drop a binary or image archive</p>
              <p className="mt-1 text-xs text-muted">
                ELF binary, Docker save, or OCI archive — identified from magic bytes.{" "}
                {INSPECTION_ONLY_LABEL}: uploads never start a demo optimizer or invent benchmarks.
                Native optimize requires a quench.yaml in the workspace.
              </p>
            </>
          )}
          <input
            ref={inputRef}
            type="file"
            className="sr-only"
            disabled={reading}
            onChange={(e) => void onFiles(e.target.files)}
          />
        </label>
        {uploadError ? (
          <div
            role="alert"
            className="mt-3 rounded-xl bg-danger/10 px-4 py-3 text-sm text-danger shadow-[var(--shadow-border)]"
          >
            <p>{uploadError}</p>
            <p className="mt-1 text-xs text-muted">
              No run was created. Try a different ELF binary, Docker save, or OCI archive.
            </p>
          </div>
        ) : null}

        <h2 className="mt-10 font-display text-lg font-semibold">Sample workloads</h2>
        <p className="mt-2 text-xs text-muted">
          These cards play labeled demo data. They are not native agent results.
        </p>
        <div className="mt-4 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
          {SAMPLES.map((s) => (
            <button
              key={s.id}
              type="button"
              onClick={() => launch(s)}
              className="rounded-xl bg-surface p-4 text-left shadow-[var(--shadow-border)] transition-[box-shadow] duration-150 hover:shadow-[var(--shadow-border-hover)]"
            >
              <div className="flex items-center justify-between">
                {s.kind === "docker" ? (
                  <Box className="size-4 text-accent" />
                ) : (
                  <Cpu className="size-4 text-accent" />
                )}
                <Badge variant="warn">{DEMO_LABEL}</Badge>
              </div>
              <p className="mt-4 font-mono text-sm text-fg">{s.name}</p>
              <p className="mt-1 text-xs text-muted">{s.subtitle}</p>
              <p className="mt-4 font-mono text-xs text-subtle">
                {s.language} · {formatBytes(s.sizeBytes)}
              </p>
            </button>
          ))}
        </div>

        <h2 className="mt-10 font-display text-lg font-semibold">Recent runs</h2>
        {!hydrated ? (
          <p className="mt-3 text-sm text-muted">Loading history…</p>
        ) : runs.length === 0 ? (
          <p className="mt-3 text-sm text-muted">
            No runs yet. Use Run native sample when the agent is connected, or open a demo workload.
          </p>
        ) : (
          <ul className="mt-4 divide-y divide-border overflow-hidden rounded-xl bg-surface shadow-[var(--shadow-border)]">
            {runs.map((run) => {
              return (
                <li key={run.id} className="flex items-center gap-3 px-4 py-3">
                  <Link to="/studio/$runId" params={{ runId: run.id }} className="min-h-11 min-w-0 flex-1">
                    <p className="truncate font-mono text-sm">{run.artifact.name}</p>
                    <p className="text-xs text-muted">
                      {runStatusLabel(run)} · {run.mode} · {relativeTime(run.createdAt)}
                    </p>
                  </Link>
                  <button
                    type="button"
                    className="inline-flex size-11 items-center justify-center text-subtle hover:text-fg"
                    aria-label={`Remove ${run.artifact.name}`}
                    onClick={() => removeRun(run.id)}
                  >
                    <Trash2 className="size-4" />
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </main>
    </Shell>
  );
}
