import { Badge } from "@/components/ui/badge";
import { NATIVE_DISCLAIMER } from "@/lib/quench/data";
import { formatBytes, formatSha256 } from "@/lib/quench/format";
import type { NativeReport, Run } from "@/lib/quench/types";
import { cn } from "@/lib/utils";

function nativeOf(run: Run): NativeReport {
  return run.result.native ?? {};
}

function fmtMs(value: number | null | undefined): string {
  if (value == null || Number.isNaN(Number(value))) return "—";
  const n = Number(value);
  return Number.isInteger(n) ? String(n) : n.toFixed(3);
}

function toolLine(id: string, info: { status?: string; version?: string; detail?: string; path?: string }): string {
  const status =
    info.status === "ok" ? "OK" : info.status === "unavailable" ? "Unavailable" : (info.status ?? "—");
  const ver = info.version ? ` ${info.version}` : "";
  const detail = info.detail && info.status === "unavailable" ? ` — ${info.detail}` : "";
  return `${id}: ${status}${ver}${detail}`;
}

export function nativeLimitations(n: NativeReport): string[] {
  const out: string[] = [];
  for (const t of n.transformsFailed ?? []) {
    if (t.status.toLowerCase() === "unavailable") {
      out.push(`${t.tool}: Unavailable — ${t.detail}`);
    }
  }
  if (n.toolVersions) {
    for (const [id, info] of Object.entries(n.toolVersions)) {
      if (info.status === "unavailable") {
        const line = `${id}: Unavailable${info.detail ? ` — ${info.detail}` : ""}`;
        if (!out.some((x) => x.startsWith(`${id}:`))) out.push(line);
      }
    }
  }
  if (!n.keptCandidate) {
    out.push(n.reason ?? "No candidate was produced or kept.");
  }
  return out;
}

function Fact({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div>
      <dt className="text-[11px] uppercase tracking-wider text-subtle">{label}</dt>
      <dd className={cn("mt-1 break-all text-sm", mono && "font-mono text-xs leading-relaxed")}>{value}</dd>
    </div>
  );
}

export function NativeFacts({ run }: { run: Run }) {
  const n = nativeOf(run);
  const kept = Boolean(n.keptCandidate);
  const baseBytes = n.artifactSize?.baselineBytes;
  const candBytes = n.artifactSize?.candidateBytes;
  const tools = n.toolVersions
    ? Object.entries(n.toolVersions).map(([id, info]) => toolLine(id, info))
    : [];
  return (
    <>
      <p className="text-xs uppercase tracking-wider text-subtle">Native optimization report</p>
      <dl className="mt-3 space-y-3">
        <Fact label="Status" value={run.status.replaceAll("_", " ")} />
        <Fact label="Project" value={n.project ?? run.artifact.name} />
        <Fact label="Kind" value={n.kind ?? run.artifact.kind} />
        <Fact label="Input path" value={n.inputPath ?? run.artifact.tag} mono />
        <Fact label="Run ID" value={run.id} mono />
        <Fact label="Candidate" value={kept ? "Kept" : "Not kept"} />
        {n.reason ? <Fact label="Why" value={n.reason} /> : null}
        <Fact label="Tests" value={n.testResult ?? "Supplied test suite was not recorded"} />
        <Fact
          label="Baseline SHA-256"
          value={n.baselineSha256 ? formatSha256(n.baselineSha256) : "—"}
          mono
        />
        <Fact
          label="Candidate SHA-256"
          value={n.candidateSha256 ? formatSha256(n.candidateSha256) : "—"}
          mono
        />
        <Fact
          label="Artifact size"
          value={
            baseBytes != null
              ? `${formatBytes(baseBytes)} → ${candBytes != null ? formatBytes(candBytes) : "no candidate"}`
              : "—"
          }
        />
        <Fact label="Build" value={n.buildCommand ?? "—"} mono />
        <Fact label="Test" value={n.testCommand ?? "—"} mono />
        <Fact label="Profile" value={n.profileCommand ?? "—"} mono />
        <Fact
          label="Profile mode"
          value={
            n.profileMode
              ? `${n.profileMode}${n.profileReason ? ` — ${n.profileReason}` : ""}`
              : "—"
          }
        />
        {n.lbrProbeCommand ? (
          <Fact
            label="LBR probe"
            value={`${n.lbrProbeCommand}${n.lbrProbeOk === false ? " (failed)" : n.lbrProbeOk === true ? " (ok)" : ""}`}
            mono
          />
        ) : null}
        {n.profileWarning ? <Fact label="Profile warning" value={n.profileWarning} /> : null}
        <Fact
          label="Benchmarked"
          value={`original=${n.benchmarkedOriginal ?? "—"} · candidate=${n.benchmarkedCandidate ?? "—"} · instrumented=${n.benchmarkedInstrumented ?? false}`}
        />
        <Fact label="Benchmark" value={n.benchmarkCommand ?? "—"} mono />
        <Fact
          label="Median / p95"
          value={`median ${fmtMs(n.medianMs?.baseline)} → ${fmtMs(n.medianMs?.candidate)} ms · p95 ${fmtMs(n.p95Ms?.baseline)} → ${fmtMs(n.p95Ms?.candidate)} ms`}
        />
        <Fact label="Repetitions" value={n.benchmarkRepetitions != null ? String(n.benchmarkRepetitions) : "—"} />
        {n.minImprovementPercent != null ? (
          <Fact
            label="Gates"
            value={`min_improvement ${n.minImprovementPercent}% · max_regression ${n.maxRegressionPercent ?? "—"}%${
              n.medianImprovementPercent != null
                ? ` · median improvement ${n.medianImprovementPercent.toFixed(3)}%`
                : ""
            }`}
          />
        ) : null}
        {tools.length ? <Fact label="Tools" value={tools.join(" · ")} /> : null}
      </dl>
    </>
  );
}

export function NativeVerifyPanel({ run }: { run: Run }) {
  const n = nativeOf(run);
  const applied = n.transformsApplied ?? [];
  const failed = n.transformsFailed ?? [];
  const commands = n.commands ?? [];
  const limitations = nativeLimitations(n);
  return (
    <section className="mt-4 space-y-4">
      <section className="min-w-0 overflow-hidden rounded-xl bg-surface p-4 shadow-[var(--shadow-border)] sm:p-5">
        <h2 className="font-display text-lg font-semibold">Supplied test suite</h2>
        <p className="mt-1 text-xs text-subtle">What the agent actually ran — not functional equivalence.</p>
        <p className="mt-3 text-sm">{n.testResult ?? "No test result recorded."}</p>
        {n.testCommand ? <p className="mt-2 font-mono text-xs text-muted">{n.testCommand}</p> : null}
      </section>
      <section className="min-w-0 overflow-hidden rounded-xl bg-surface p-4 shadow-[var(--shadow-border)] sm:p-5">
        <h2 className="font-display text-lg font-semibold">Transforms</h2>
        <p className="mt-1 text-xs text-subtle">Missing tools are Unavailable. Success is not invented.</p>
        <ul className="mt-3 space-y-2">
          {applied.length === 0 && failed.length === 0 ? (
            <li className="text-sm text-muted">No transforms were recorded.</li>
          ) : null}
          {applied.map((t) => (
            <li key={`a-${t.tool}-${t.detail}`} className="rounded-md bg-surface-2 px-3 py-2">
              <p className="font-mono text-xs text-signal">
                {t.tool} · {t.status}
              </p>
              <p className="mt-1 text-sm">{t.detail}</p>
            </li>
          ))}
          {failed.map((t) => (
            <li key={`f-${t.tool}-${t.detail}`} className="rounded-md bg-surface-2 px-3 py-2">
              <p className="font-mono text-xs text-warn">
                {t.tool} · {t.status}
              </p>
              <p className="mt-1 text-sm">{t.detail}</p>
            </li>
          ))}
        </ul>
      </section>
      {commands.length ? (
        <section className="min-w-0 overflow-hidden rounded-xl bg-surface p-4 shadow-[var(--shadow-border)] sm:p-5">
          <h2 className="font-display text-lg font-semibold">Commands</h2>
          <p className="mt-1 text-xs text-subtle">Real subprocesses, with versions when the tool reported one.</p>
          <ul className="mt-3 max-h-64 space-y-2 overflow-auto">
            {commands.map((c, i) => (
              <li key={`${c.command}-${i}`} className="rounded-md bg-bg px-3 py-2 font-mono text-[11px]">
                <p className="break-all text-fg">{c.command}</p>
                <p className="mt-1 text-subtle">
                  exit {c.exit_code} · {c.duration_ms} ms
                  {c.tool_version ? ` · ${c.tool_version}` : ""}
                  {c.timed_out ? " · timed out" : ""}
                </p>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
      {limitations.length ? (
        <section className="min-w-0 overflow-hidden rounded-xl bg-surface p-4 shadow-[var(--shadow-border)] sm:p-5">
          <h2 className="font-display text-lg font-semibold">Limitations</h2>
          <ul className="mt-3 space-y-2 text-sm text-muted">
            {limitations.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </section>
      ) : null}
    </section>
  );
}

export function NativeReportArticle({ run }: { run: Run }) {
  const n = nativeOf(run);
  const kept = Boolean(n.keptCandidate);
  const limitations = nativeLimitations(n);
  const tools = n.toolVersions ? Object.entries(n.toolVersions) : [];
  return (
    <article className="mt-4 rounded-2xl bg-surface p-6 shadow-[var(--shadow-border)] sm:p-10">
      <div className="flex items-start justify-between gap-4">
        <p className="text-xs uppercase tracking-[0.18em] text-subtle">Local optimization report</p>
        <Badge
          variant={
            run.status === "failed" || run.status === "verification_failed"
              ? "danger"
              : kept
                ? "signal"
                : "warn"
          }
        >
          {run.status === "failed" || run.status === "verification_failed"
            ? run.status === "verification_failed"
              ? "Tests failed"
              : "Failed"
            : kept
              ? "Candidate kept"
              : "Candidate not kept"}
        </Badge>
      </div>
      <h1 className="mt-3 font-display text-3xl font-semibold">{run.artifact.name}</h1>
      <p className="mt-1 font-mono text-xs text-subtle">{run.id}</p>
      <p className="mt-4 text-muted">{run.result.summary}</p>
      {run.status === "failed" || run.status === "verification_failed" ? (
        <p role="alert" className="mt-2 text-sm text-danger">
          {n.error ?? "The native agent reported a failed run."}
        </p>
      ) : null}
      <p className="mt-2 text-sm">Status: {run.status.replaceAll("_", " ")}</p>

      <dl className="mt-8 grid grid-cols-1 gap-4 sm:grid-cols-2">
        <Fact label="Tests" value={n.testResult ?? "—"} />
        <Fact
          label="Size"
          value={
            n.artifactSize?.baselineBytes != null
              ? `${formatBytes(n.artifactSize.baselineBytes)} → ${
                  n.artifactSize.candidateBytes != null
                    ? formatBytes(n.artifactSize.candidateBytes)
                    : "no candidate"
                }`
              : "—"
          }
        />
        <Fact
          label="Median ms"
          value={`${fmtMs(n.medianMs?.baseline)} → ${fmtMs(n.medianMs?.candidate)}`}
        />
        <Fact label="p95 ms" value={`${fmtMs(n.p95Ms?.baseline)} → ${fmtMs(n.p95Ms?.candidate)}`} />
        <Fact label="Repetitions" value={n.benchmarkRepetitions != null ? String(n.benchmarkRepetitions) : "—"} />
        <Fact
          label="Gates"
          value={`min_improvement ${n.minImprovementPercent ?? "—"}% · max_regression ${n.maxRegressionPercent ?? "—"}%`}
        />
        <div className="sm:col-span-2">
          <Fact label="Baseline SHA-256" value={n.baselineSha256 ? formatSha256(n.baselineSha256) : "—"} mono />
        </div>
        <div className="sm:col-span-2">
          <Fact label="Candidate SHA-256" value={n.candidateSha256 ? formatSha256(n.candidateSha256) : "—"} mono />
        </div>
      </dl>

      {n.buildCommand || n.testCommand || n.benchmarkCommand || n.profileCommand || n.reproducibleCommand ? (
        <div className="mt-8 border-t border-border pt-6 font-mono text-xs text-muted">
          {n.reproducibleCommand ? <p className="break-all text-fg">{n.reproducibleCommand}</p> : null}
          {n.buildCommand ? <p className="mt-2 break-all">build {n.buildCommand}</p> : null}
          {n.testCommand ? <p className="mt-1 break-all">test {n.testCommand}</p> : null}
          {n.profileCommand ? <p className="mt-1 break-all">profile {n.profileCommand}</p> : null}
          {n.benchmarkCommand ? <p className="mt-1 break-all">bench {n.benchmarkCommand}</p> : null}
        </div>
      ) : null}

      {(n.transformsApplied?.length || n.transformsFailed?.length) ? (
        <div className="mt-8 border-t border-border pt-6">
          <p className="text-xs uppercase tracking-wider text-subtle">Transforms</p>
          <ul className="mt-3 space-y-2 text-sm">
            {(n.transformsApplied ?? []).map((t) => (
              <li key={`a-${t.tool}`}>
                {t.tool} · {t.status} — {t.detail}
              </li>
            ))}
            {(n.transformsFailed ?? []).map((t) => (
              <li key={`f-${t.tool}`} className="text-muted">
                {t.tool} · {t.status} — {t.detail}
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      {(n.commands ?? []).length ? (
        <div className="mt-8 border-t border-border pt-6">
          <p className="text-xs uppercase tracking-wider text-subtle">Commands</p>
          <ul className="mt-3 space-y-2 font-mono text-xs text-muted">
            {(n.commands ?? []).map((c, i) => (
              <li key={`${c.command}-${i}`}>
                {c.command} · exit {c.exit_code} · {c.duration_ms} ms
                {c.tool_version ? ` · ${c.tool_version}` : ""}
              </li>
            ))}
          </ul>
        </div>
      ) : null}

      {tools.length ? (
        <div className="mt-8 border-t border-border pt-6">
          <p className="text-xs uppercase tracking-wider text-subtle">Tool versions</p>
          <ul className="mt-3 space-y-1 font-mono text-xs text-muted">
            {tools.map(([id, info]) => (
              <li key={id}>{toolLine(id, info)}</li>
            ))}
          </ul>
        </div>
      ) : null}

      {limitations.length ? (
        <div className="mt-8 border-t border-border pt-6">
          <p className="text-xs uppercase tracking-wider text-subtle">Limitations</p>
          <ul className="mt-3 space-y-2 text-sm text-muted">
            {limitations.map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </div>
      ) : null}

      <p className="mt-8 text-xs text-subtle">{n.disclaimer ?? NATIVE_DISCLAIMER}</p>
    </article>
  );
}
