import { createFileRoute, Link } from "@tanstack/react-router";
import { useEffect } from "react";
import { ArrowLeft } from "lucide-react";
import { Logo } from "@/components/brand/logo";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Shell } from "@/components/layout/shell";
import { NativeReportArticle } from "@/components/studio/native-report";
import {
  DEMO_DISCLAIMER,
  DEMO_LABEL,
  INSPECTION_ONLY_LABEL,
  INSPECT_DISCLAIMER,
  MODELED_LABEL,
} from "@/lib/quench/data";
import { activeMetrics, isAgentRun, isDemo } from "@/lib/quench/engine";
import { formatBytes, formatPct, formatSha256, formatUsd, improvePct, signedPct } from "@/lib/quench/format";
import { useQuenchStore } from "@/lib/quench/store";

export const Route = createFileRoute("/cert/$runId")({ component: CertPage });

function CertPage() {
  const { runId } = Route.useParams();
  const run = useQuenchStore((s) => s.runs.find((r) => r.id === runId));
  const hydrated = useQuenchStore((s) => s.hydrated);
  const setHydrated = useQuenchStore((s) => s.setHydrated);

  useEffect(() => {
    if (!hydrated) setHydrated();
  }, [hydrated, setHydrated]);

  if (!hydrated) {
    return (
      <Shell>
        <main className="px-6 py-16 text-sm text-muted">Loading report…</main>
      </Shell>
    );
  }

  if (!run) {
    return (
      <Shell>
        <main className="mx-auto max-w-lg px-4 py-24 text-center">
          <h1 className="font-display text-2xl">Report not found</h1>
          <p className="mt-2 text-sm text-muted">Open it from Studio in this browser, then reload.</p>
          <Button asChild className="mt-6">
            <Link to="/studio">Studio</Link>
          </Button>
        </main>
      </Shell>
    );
  }

  const demo = isDemo(run);
  const agentRun = isAgentRun(run);
  const before = run.result.before;
  const after = activeMetrics(run);
  const kind =
    run.artifact.kind === "elf" ? "ELF" : run.artifact.kind === "oci" ? "OCI" : "Docker";

  return (
    <Shell>
      <main className="mx-auto max-w-3xl px-4 py-10 sm:px-6">
        <Link
          to="/studio/$runId"
          params={{ runId: run.id }}
          className="inline-flex h-11 items-center gap-2 text-sm text-muted hover:text-fg"
        >
          <ArrowLeft className="size-4" />
          Back to run
        </Link>

        {agentRun ? (
          <NativeReportArticle run={run} />
        ) : (
          <article className="mt-4 rounded-2xl bg-surface p-6 shadow-[var(--shadow-border)] sm:p-10">
            <div className="flex items-start justify-between gap-4">
              <Logo />
              <div className="flex flex-col items-end gap-2">
                {demo ? <Badge variant="warn">{DEMO_LABEL}</Badge> : <Badge variant="warn">{INSPECTION_ONLY_LABEL}</Badge>}
                <p className="font-mono text-xs text-subtle">
                  {demo ? "Demo data · modeled result · not executed" : "File identity only"}
                </p>
              </div>
            </div>
            <p className="mt-8 text-xs uppercase tracking-[0.18em] text-subtle">
              {demo ? "Demo optimization report" : "Inspection report"}
            </p>
            <h1 className="mt-2 font-display text-3xl font-semibold">{run.result.certId}</h1>
            <p className="mt-4 text-muted">{run.result.summary}</p>

            {demo ? (
              <dl className="mt-8 grid grid-cols-2 gap-4 sm:grid-cols-4">
                <CertStat
                  label="Artifact"
                  value={signedPct(before.artifactBytes, after.artifactBytes)}
                  sub={`${formatBytes(before.artifactBytes)} → ${formatBytes(after.artifactBytes)}`}
                />
                <CertStat
                  label="Throughput"
                  value={formatPct(improvePct(before.throughputRps, after.throughputRps, true))}
                  sub={MODELED_LABEL}
                />
                <CertStat
                  label="Memory"
                  value={signedPct(before.rssMb, after.rssMb)}
                  sub={`${before.rssMb} → ${after.rssMb} MiB`}
                />
                <CertStat
                  label="SCI"
                  value={signedPct(before.sci, after.sci)}
                  sub={`${before.sci} → ${after.sci}`}
                />
              </dl>
            ) : (
              <dl className="mt-8 grid grid-cols-1 gap-4 sm:grid-cols-2">
                <CertStat label="Type" value={kind} sub={run.artifact.subtitle} />
                <CertStat
                  label="Size"
                  value={formatBytes(run.artifact.sizeBytes)}
                  sub={`${run.artifact.sizeBytes} bytes`}
                />
                <div className="sm:col-span-2">
                  <dt className="text-[11px] uppercase tracking-wider text-subtle">SHA-256</dt>
                  <dd className="mt-1 break-all font-mono text-sm">
                    {run.artifact.sha256 ? formatSha256(run.artifact.sha256) : "unavailable"}
                  </dd>
                </div>
              </dl>
            )}

            <div className="mt-8 grid gap-6 border-t border-border pt-6 text-sm sm:grid-cols-2">
              <div>
                <p className="text-xs uppercase tracking-wider text-subtle">Subject</p>
                <p className="mt-1 font-mono">{run.artifact.name}</p>
                <p className="text-muted">{run.artifact.tag}</p>
                <p className="mt-2 text-muted">
                  {kind} · {run.artifact.language}
                  {demo ? ` · Pareto ${run.preference}` : ""}
                </p>
              </div>
              <div>
                <p className="text-xs uppercase tracking-wider text-subtle">
                  {demo ? "Sample verification" : "What was checked"}
                </p>
                {demo ? (
                  <>
                    <p className="mt-1">
                      {run.result.tests.filter((t) => t.status === "pass").length}/{run.result.tests.length} sample
                      checks labeled pass
                    </p>
                    <p className="mt-2 text-muted">
                      Modeled monthly {formatUsd(before.monthlyUsd)} → {formatUsd(after.monthlyUsd)}
                    </p>
                  </>
                ) : (
                  <p className="mt-1 text-muted">
                    Magic bytes, archive structure, size, and SHA-256 of the uploaded bytes. Bloaty, BOLT,
                    UPX, containerd, and perf were not run. This is not an optimization run.
                  </p>
                )}
                <p className="mt-2 text-muted">
                  Issued {new Date(run.createdAt).toISOString().slice(0, 10)} · this browser
                </p>
              </div>
            </div>

            <p className="mt-8 text-xs text-subtle">{demo ? DEMO_DISCLAIMER : INSPECT_DISCLAIMER}</p>
          </article>
        )}
      </main>
    </Shell>
  );
}

function CertStat({ label, value, sub }: { label: string; value: string; sub: string }) {
  return (
    <div>
      <dt className="text-[11px] uppercase tracking-wider text-subtle">{label}</dt>
      <dd className="mt-1 font-display text-2xl tabular-nums text-signal">{value}</dd>
      <p className="text-[11px] text-muted">{sub}</p>
    </div>
  );
}
