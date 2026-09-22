import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { useEffect, useMemo, useRef, useState, type ReactNode, type RefObject } from "react";
import { ArrowLeft, Check, Copy, Download } from "lucide-react";
import { toast, Toaster } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Shell } from "@/components/layout/shell";
import { BeforeAfterBars, HeadroomRadar, LayerBars } from "@/components/studio/charts";
import { Pipeline } from "@/components/studio/pipeline";
import { AgentBadge } from "@/components/studio/agent-status";
import { NativeFacts, NativeVerifyPanel } from "@/components/studio/native-report";
import { DEMO_DISCLAIMER, DEMO_LABEL, INSPECTION_ONLY_LABEL, INSPECT_DISCLAIMER, MODELED_LABEL, NATIVE_DISCLAIMER, PREF_COPY } from "@/lib/quench/data";
import {
  activeMetrics,
  isAgentRun,
  isDemo,
  isInspect,
  revealedLogs,
  revealedStage,
  runStatusLabel,
  stageProgress,
  stagesFor,
} from "@/lib/quench/engine";
import {
  formatBytes,
  formatMs,
  formatPct,
  formatSha256,
  formatUsd,
  improvePct,
  signedPct,
} from "@/lib/quench/format";
import { useQuenchStore } from "@/lib/quench/store";
import type { ArtifactKind, ParetoPref, Run } from "@/lib/quench/types";
import { badgeUnavailableReason, reportPublicUrl } from "@/lib/quench/urls";
import { cn } from "@/lib/utils";

export const Route = createFileRoute("/studio/$runId")({ component: RunPage });

function kindBadge(kind: ArtifactKind): string {
  if (kind === "elf") return "ELF";
  if (kind === "oci") return "OCI";
  return "Docker";
}

function RunPage() {
  const { runId } = Route.useParams();
  const navigate = useNavigate();
  const run = useQuenchStore((s) => s.runs.find((r) => r.id === runId));
  const markCompleteIfDue = useQuenchStore((s) => s.markCompleteIfDue);
  const setRunPreference = useQuenchStore((s) => s.setRunPreference);
  const hydrated = useQuenchStore((s) => s.hydrated);
  const setHydrated = useQuenchStore((s) => s.setHydrated);
  const [now, setNow] = useState(() => Date.now());
  const logRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!hydrated) setHydrated();
  }, [hydrated, setHydrated]);

  useEffect(() => {
    if (!run || run.status === "complete") return;
    const id = window.setInterval(() => {
      setNow(Date.now());
      markCompleteIfDue(run.id);
    }, 80);
    return () => window.clearInterval(id);
  }, [run, markCompleteIfDue]);

  const stages = run ? stagesFor(run.mode) : [];
  const stage = run ? revealedStage(run, now) : 0;
  const logs = useMemo(() => (run ? revealedLogs(run, now) : []), [run, now]);

  useEffect(() => {
    const el = logRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [logs.length]);

  if (!hydrated) {
    return (
      <Shell wide>
        <main className="px-6 py-16 text-sm text-muted">Restoring run…</main>
      </Shell>
    );
  }

  if (!run) {
    return (
      <Shell wide>
        <main className="mx-auto max-w-lg px-4 py-24 text-center">
          <h1 className="font-display text-2xl font-semibold">Run not found</h1>
          <p className="mt-2 text-sm text-muted">It may have been cleared from local history.</p>
          <Button asChild className="mt-6">
            <Link to="/studio">Back to studio</Link>
          </Button>
        </main>
      </Shell>
    );
  }

  const demo = isDemo(run);
  const agentRun = isAgentRun(run);
  const after = activeMetrics(run);
  const before = run.result.before;
  const showAnalysis = demo && stage >= 2;
  const showOptimize = demo && stage >= 5;
  const showVerify = (demo && stage >= 6) || (agentRun && run.status !== "running");
  const showBench = demo && stage >= 7;
  const showProve = stage >= stages.length || run.status !== "running";
  const { index, frac } = stageProgress(run, now);
  const overall =
    run.status === "running" ? Math.min(1, (index + frac) / Math.max(1, stages.length)) : 1;
  const statusLabel = runStatusLabel(run);

  return (
    <Shell wide>
      <Toaster theme="dark" position="bottom-right" />
      <main className="mx-auto max-w-7xl px-4 py-6 sm:px-6">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
          <div className="min-w-0">
            <Link
              to="/studio"
              className="inline-flex h-11 items-center gap-2 text-sm text-muted hover:text-fg"
            >
              <ArrowLeft className="size-4" />
              Studio
            </Link>
            <h1 className="font-display text-2xl font-semibold sm:text-3xl">{run.artifact.name}</h1>
            <p className="mt-1 font-mono text-xs text-subtle">{run.artifact.subtitle}</p>
            <div className="mt-3 flex flex-wrap gap-2">
              <Badge>{kindBadge(run.artifact.kind)}</Badge>
              <Badge>{run.artifact.language}</Badge>
              <Badge
                variant={
                  run.status === "running"
                    ? "accent"
                    : agentRun && (run.status === "failed" || run.status === "verification_failed")
                      ? "danger"
                      : agentRun && run.status !== "complete"
                        ? "warn"
                        : "signal"
                }
              >
                {statusLabel}
              </Badge>
              {demo ? (
                <Badge variant="warn">{DEMO_LABEL}</Badge>
              ) : agentRun ? (
                <AgentBadge agent={null} run={run} />
              ) : (
                <Badge variant="warn">{INSPECTION_ONLY_LABEL}</Badge>
              )}
            </div>
          </div>
          {demo ? (
            <div className="flex flex-wrap gap-2">
              {(Object.keys(PREF_COPY) as ParetoPref[]).map((key) => (
                <Button
                  key={key}
                  size="sm"
                  className="shrink-0"
                  variant={run.preference === key ? "default" : "outline"}
                  onClick={() => setRunPreference(run.id, key)}
                >
                  {PREF_COPY[key].label}
                </Button>
              ))}
            </div>
          ) : null}
        </div>

        {agentRun && (run.status === "failed" || run.status === "verification_failed") ? (
          <div
            role="alert"
            className="mt-4 rounded-xl bg-danger/10 px-4 py-3 text-sm text-danger shadow-[var(--shadow-border)]"
          >
            <p className="font-medium">
              {run.status === "verification_failed" ? "Verification failed" : "Native optimize failed"}
            </p>
            <p className="mt-1">
              {run.result.native?.error ?? run.result.summary ?? "The native agent reported a failed run."}
            </p>
          </div>
        ) : null}

        <div className="mt-6">
          <div className="mb-3 flex items-center justify-between text-xs text-muted">
            <span>
              {run.status === "running"
                ? (stages[Math.min(index, stages.length - 1)]?.label ?? "")
                : demo
                  ? "Demo pipeline complete"
                  : agentRun
                    ? run.status === "failed" || run.status === "verification_failed"
                      ? "Native pipeline failed"
                      : "Native pipeline finished"
                    : "Inspection complete"}
            </span>
            <span className="tabular-nums">{Math.round(overall * 100)}%</span>
          </div>
          <Pipeline run={run} now={now} />
        </div>

        <div className="mt-6 grid gap-4 lg:grid-cols-5">
          <div className="min-w-0 lg:col-span-3">
            <LogPanel
              logs={logs}
              logRef={logRef}
              title={
                demo
                  ? "quench · demo orchestrator"
                  : agentRun
                    ? "quench-agent · native pipeline"
                    : "quench · local inspector"
              }
            />
          </div>
          <aside className="min-w-0 rounded-xl bg-surface p-4 shadow-[var(--shadow-border)] lg:col-span-2">
            {demo ? (
              <>
                <div className="flex items-center justify-between gap-2">
                  <p className="text-xs uppercase tracking-wider text-subtle">Headline deltas</p>
                  <Badge variant="warn">{MODELED_LABEL}</Badge>
                </div>
                <dl className="mt-3 grid grid-cols-2 gap-3">
                  <Delta
                    label="Artifact"
                    value={signedPct(before.artifactBytes, after.artifactBytes)}
                    sub={`${formatBytes(before.artifactBytes)} → ${formatBytes(after.artifactBytes)}`}
                    ready={showOptimize}
                  />
                  <Delta
                    label="Throughput"
                    value={formatPct(improvePct(before.throughputRps, after.throughputRps, true))}
                    sub={`${before.throughputRps.toLocaleString()} → ${after.throughputRps.toLocaleString()} rps`}
                    ready={showBench}
                  />
                  <Delta
                    label="RSS"
                    value={signedPct(before.rssMb, after.rssMb)}
                    sub={`${before.rssMb} → ${after.rssMb} MiB`}
                    ready={showOptimize}
                  />
                  <Delta
                    label="Est. monthly"
                    value={signedPct(before.monthlyUsd, after.monthlyUsd)}
                    sub={`${formatUsd(before.monthlyUsd)} → ${formatUsd(after.monthlyUsd)}`}
                    ready={showProve}
                  />
                </dl>
              </>
            ) : agentRun ? (
              <NativeFacts run={run} />
            ) : (
              <VerifiedFacts run={run} />
            )}
          </aside>
        </div>

        {showAnalysis ? (
          <section className="mt-8 grid min-w-0 gap-4 lg:grid-cols-2">
            <Card title="Optimization headroom" hint={`${MODELED_LABEL}. Score vs speed-of-light on each axis.`}>
              <HeadroomRadar headroom={run.result.headroom} />
              <ul className="mt-2 space-y-2">
                {run.result.headroom.map((h) => (
                  <li key={h.id} className="text-xs text-muted">
                    <span className="font-medium text-fg">{h.label}.</span> {h.note}
                  </li>
                ))}
              </ul>
            </Card>
            <Card title="Layer map" hint={`${MODELED_LABEL}. What the sample image occupied.`}>
              <LayerBars layers={run.result.layers} />
            </Card>
          </section>
        ) : null}

        {showOptimize ? (
          <section className="mt-4">
            <Card title="Passes applied" hint={`${MODELED_LABEL}. ${PREF_COPY[run.preference].hint}`}>
              <ul className="grid gap-2 sm:grid-cols-2">
                {run.result.tools.map((t) => (
                  <li
                    key={`${t.tool}-${t.action}`}
                    className="rounded-md bg-surface-2 px-3 py-2 shadow-[var(--shadow-border)]"
                  >
                    <p className="font-mono text-xs text-accent">{t.tool}</p>
                    <p className="mt-1 text-sm">{t.action}</p>
                  </li>
                ))}
              </ul>
            </Card>
          </section>
        ) : null}

        {showVerify ? demo ? <VerifyPanel run={run} /> : agentRun ? <NativeVerifyPanel run={run} /> : null : null}

        {showBench ? (
          <section className="mt-4 grid min-w-0 gap-4 lg:grid-cols-2">
            <Card title="Before / after" hint={`${MODELED_LABEL}. Sample harness, not a live benchmark.`}>
              <BeforeAfterBars before={before} after={after} />
              <div className="mt-2 grid grid-cols-2 gap-2 text-xs sm:grid-cols-4">
                <Mini label="p99" value={formatMs(after.p99Ms)} />
                <Mini label="Cold start" value={formatMs(after.coldStartMs)} />
                <Mini label="CPU cores" value={String(after.cpuCores)} />
                <Mini label="Vulns" value={`${after.vulns} (was ${before.vulns})`} />
              </div>
            </Card>
            <Card title="Kubernetes patch" hint={`${MODELED_LABEL}. Example requests, not measured p95.`}>
              <div className="grid min-w-0 gap-3 sm:grid-cols-2">
                <pre className="overflow-x-auto rounded-md bg-bg p-3 font-mono text-xs leading-relaxed text-muted">
                  {run.result.k8sBefore}
                </pre>
                <pre className="overflow-x-auto rounded-md bg-bg p-3 font-mono text-xs leading-relaxed text-signal">
                  {run.result.k8sAfter}
                </pre>
              </div>
            </Card>
          </section>
        ) : null}

        {showProve ? (
          <ProvePanel
            run={run}
            onOpenCert={() => {
              void navigate({ to: "/cert/$runId", params: { runId: run.id } });
            }}
          />
        ) : null}
      </main>
    </Shell>
  );
}

function VerifiedFacts({ run }: { run: Run }) {
  const artifact = run.artifact;
  return (
    <>
      <p className="text-xs uppercase tracking-wider text-subtle">Inspection only</p>
      <dl className="mt-3 space-y-3">
        <Fact label="Name" value={artifact.name} />
        <Fact label="Type" value={kindBadge(artifact.kind)} />
        <Fact label="Size" value={`${formatBytes(artifact.sizeBytes)} · ${artifact.sizeBytes} bytes`} />
        <Fact
          label="SHA-256"
          value={artifact.sha256 ? formatSha256(artifact.sha256) : "unavailable"}
          mono
        />
        {artifact.elf ? (
          <Fact
            label="ELF"
            value={`ELF${artifact.elf.classBits} ${artifact.elf.endian}-endian ${artifact.elf.machine}`}
          />
        ) : null}
        {artifact.archive?.repoTags?.length ? (
          <Fact label="Tags" value={artifact.archive.repoTags.join(", ")} />
        ) : null}
      </dl>
    </>
  );
}

function Fact({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div>
      <dt className="text-[11px] uppercase tracking-wider text-subtle">{label}</dt>
      <dd className={cn("mt-1 break-all text-sm", mono && "font-mono text-xs leading-relaxed")}>{value}</dd>
    </div>
  );
}

function Card({
  title,
  hint,
  children,
}: {
  title: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <section className="min-w-0 overflow-hidden rounded-xl bg-surface p-4 shadow-[var(--shadow-border)] sm:p-5">
      <div className="mb-3">
        <h2 className="font-display text-lg font-semibold">{title}</h2>
        {hint ? <p className="mt-1 text-xs text-subtle">{hint}</p> : null}
      </div>
      {children}
    </section>
  );
}

function Delta({
  label,
  value,
  sub,
  ready,
}: {
  label: string;
  value: string;
  sub: string;
  ready: boolean;
}) {
  return (
    <div>
      <dt className="text-[11px] uppercase tracking-wider text-subtle">{label}</dt>
      <dd className={cn("mt-1 font-display text-2xl tabular-nums", ready ? "text-signal" : "text-subtle")}>
        {ready ? value : "—"}
      </dd>
      <p className="text-[11px] text-muted">{ready ? sub : "awaiting stage"}</p>
    </div>
  );
}

function Mini({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md bg-bg px-2 py-2">
      <p className="text-[10px] uppercase tracking-wider text-subtle">{label}</p>
      <p className="mt-0.5 font-mono text-sm tabular-nums">{value}</p>
    </div>
  );
}

function LogPanel({
  logs,
  logRef,
  title,
}: {
  logs: { stage: string; line: string }[];
  logRef: RefObject<HTMLDivElement | null>;
  title: string;
}) {
  return (
    <div className="overflow-hidden rounded-xl bg-bg p-3 shadow-[var(--shadow-border)]">
      <p className="px-2 pb-2 font-mono text-xs text-subtle">{title}</p>
      <div
        ref={logRef}
        className="log-scroller h-56 overflow-auto px-2 font-mono text-xs leading-6 text-muted"
      >
        {logs.length === 0 ? (
          <p>waiting for analyzer…</p>
        ) : (
          logs.map((l, i) => (
            <p key={`${l.stage}-${i}`} className="flex min-w-0 gap-3">
              <span className="w-20 shrink-0 text-subtle">{l.stage}</span>
              <span className="min-w-0 break-all text-fg">{l.line}</span>
            </p>
          ))
        )}
      </div>
    </div>
  );
}

function VerifyPanel({ run }: { run: Run }) {
  return (
    <section className="mt-4">
      <Card title="Sample verification" hint={`${MODELED_LABEL}. These checks were not executed in the browser.`}>
        <ul className="space-y-2">
          {run.result.tests.map((t) => (
            <li
              key={t.name}
              className="flex items-center justify-between gap-3 rounded-md bg-surface-2 px-3 py-2"
            >
              <div className="flex items-center gap-2">
                <Check
                  className={cn(
                    "size-4",
                    t.status === "pass" ? "text-signal" : t.status === "warn" ? "text-warn" : "text-muted",
                  )}
                />
                <div>
                  <p className="text-sm">{t.name}</p>
                  <p className="text-xs text-muted">{t.detail}</p>
                </div>
              </div>
              <span className="font-mono text-xs text-subtle tabular-nums">
                {t.status === "skip" ? "skip" : `${t.durationMs} ms`}
              </span>
            </li>
          ))}
        </ul>
        <p className="mt-3 text-xs text-subtle">Sample verification passed — demo data, not a live test run.</p>
      </Card>
    </section>
  );
}

function ProvePanel({ run, onOpenCert }: { run: Run; onOpenCert: () => void }) {
  const demo = isDemo(run);
  const agentRun = isAgentRun(run);
  const after = activeMetrics(run);
  const before = run.result.before;
  const reportUrl = demo ? reportPublicUrl(run.id) : null;
  const badge = reportUrl
    ? `[![Quench demo report](https://img.shields.io/badge/quench-${run.result.certId}-7eae86)](${reportUrl})`
    : "";
  const n = run.result.native;

  function copy(text: string, ok: string) {
    void navigator.clipboard.writeText(text);
    toast(ok);
  }

  const title = demo ? "Demo optimization report" : agentRun ? "Local optimization report" : "Inspection report";
  const hint = demo
    ? `${MODELED_LABEL} · ${DEMO_LABEL}`
    : agentRun
      ? "Local agent output — not a certificate"
      : INSPECTION_ONLY_LABEL;

  return (
    <section className="mt-4 mb-10">
      <Card title={title} hint={hint}>
        <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
          <div className="min-w-0">
            <p className="font-mono text-sm text-accent">{agentRun ? run.id : run.result.certId}</p>
            <p className="mt-2 max-w-xl text-sm text-muted">{run.result.summary}</p>
            {demo ? (
              <p className="mt-3 text-sm">
                SCI {before.sci} → {after.sci} · K8s {before.k8sCpu}/{before.k8sMem} → {after.k8sCpu}/
                {after.k8sMem}
              </p>
            ) : agentRun ? (
              <div className="mt-3 space-y-1 text-xs text-muted">
                {n?.reproducibleCommand ? (
                  <p className="break-all font-mono">{n.reproducibleCommand}</p>
                ) : null}
                {n?.buildCommand ? <p>build {n.buildCommand}</p> : null}
                {n?.benchmarkCommand ? <p>bench {n.benchmarkCommand}</p> : null}
                {n?.minImprovementPercent != null ? (
                  <p>
                    min_improvement_percent {n.minImprovementPercent} · max_regression_percent{" "}
                    {n.maxRegressionPercent ?? "—"}
                    {n.medianImprovementPercent != null
                      ? ` · median improvement ${n.medianImprovementPercent.toFixed(3)}%`
                      : ""}
                  </p>
                ) : null}
                {!n?.keptCandidate ? <p>{n?.reason ?? "No candidate was kept."}</p> : null}
              </div>
            ) : run.artifact.sha256 ? (
              <p className="mt-3 break-all font-mono text-xs text-muted">
                SHA-256 {formatSha256(run.artifact.sha256)}
              </p>
            ) : null}
            <p className="mt-3 text-xs text-subtle">
              {agentRun ? (n?.disclaimer ?? NATIVE_DISCLAIMER) : isInspect(run) ? INSPECT_DISCLAIMER : DEMO_DISCLAIMER}
            </p>
          </div>
          <div className="flex flex-col gap-2">
            <Button onClick={onOpenCert}>
              <Download className="size-4" />
              Open report
            </Button>
            {reportUrl ? (
              <Button variant="outline" onClick={() => copy(badge, "README badge copied")}>
                <Copy className="size-4" />
                Copy README badge
              </Button>
            ) : (
              <div>
                <Button variant="outline" disabled>
                  <Copy className="size-4" />
                  Copy README badge
                </Button>
                <p className="mt-2 max-w-xs text-xs text-subtle">{badgeUnavailableReason(run.mode)}</p>
              </div>
            )}
          </div>
        </div>
      </Card>
    </section>
  );
}
