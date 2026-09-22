import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { useEffect, useState } from "react";
import { ArrowRight, Check, Lock, Cpu, Box, Gauge, Cloud } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Footer, Shell } from "@/components/layout/shell";
import { AgentBanner } from "@/components/studio/agent-status";
import { DIMENSIONS, PIPELINE, SAMPLE_BY_ID, DEMO_LABEL, MODELED_LABEL, NOT_EXECUTED_LABEL } from "@/lib/quench/data";
import { probeAgent, type AgentStatus } from "@/lib/quench/agent";
import { useQuenchStore } from "@/lib/quench/store";

export const Route = createFileRoute("/")({ component: Home });

const ICONS = {
  runtime: Cpu,
  artifact: Box,
  resource: Gauge,
  infra: Cloud,
} as const;

function Home() {
  const startRun = useQuenchStore((s) => s.startRun);
  const navigate = useNavigate();
  const [agent, setAgent] = useState<AgentStatus | null>(null);

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

  return (
    <Shell>
      <main>
        <section className="relative overflow-hidden">
          <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_at_top,color-mix(in_oklab,var(--color-accent)_8%,transparent),transparent_55%)]" />
          <div className="relative mx-auto max-w-6xl px-4 pb-16 pt-14 sm:px-6 sm:pt-20 lg:pb-24">
            <div className="stagger-in max-w-3xl">
              <Badge variant="accent">Local-first · Linux x86_64 ELF</Badge>
              <h1 className="mt-5 font-display text-4xl font-semibold tracking-tight text-fg sm:text-6xl">
                After the vibe, the proof.
              </h1>
              <p className="mt-5 max-w-xl text-base text-muted sm:text-lg">
                AI writes software in hours. Quench is a local Linux x86_64
                optimization pilot — build, test, transform, re-test, benchmark —
                on your machine. Public Studio is a labeled demo until the agent
                is connected.
              </p>
              <div className="mt-8 flex flex-col gap-3 sm:flex-row">
                <Button asChild size="lg">
                  <Link to="/studio">
                    Open studio
                    <ArrowRight />
                  </Link>
                </Button>
                <Button
                  size="lg"
                  variant="outline"
                  onClick={() => {
                    const artifact = SAMPLE_BY_ID["orders-api"];
                    if (!artifact) return;
                    const run = startRun(artifact, "balanced");
                    void navigate({ to: "/studio/$runId", params: { runId: run.id } });
                  }}
                >
                  View the demo story
                </Button>
              </div>
            </div>

            <div className="mt-10">
              <AgentBanner agent={agent} />
            </div>
            <HeroProof />
          </div>
        </section>

        <section className="border-t border-border">
          <div className="mx-auto grid max-w-6xl gap-10 px-4 py-16 sm:px-6 lg:grid-cols-2 lg:py-20">
            <div>
              <p className="text-xs font-medium uppercase tracking-[0.16em] text-subtle">The gap</p>
              <h2 className="mt-3 font-display text-3xl font-semibold">
                Tools exist. A closed loop does not.
              </h2>
              <p className="mt-4 text-muted">
                BOLT tunes CPU layout. Slim.ai shrinks images. CAST AI rightsizes clusters. None of
                them analyze, transform, rebuild, run the supplied test suite, and write a local
                report — then stop honestly when a tool is missing.
              </p>
            </div>
            <ul className="space-y-3">
              {[
                "Identify ELF and Docker/OCI from file bytes, not file names",
                "Run the project build, test, and benchmark as real subprocesses",
                "Apply llvm-bolt when a profile exists; otherwise report Unavailable",
                "Keep a candidate only if the supplied test suite passes and runtime does not regress",
                "Emit checksums, command logs, and a reproducible command — not a certificate",
              ].map((item) => (
                <li
                  key={item}
                  className="flex gap-3 rounded-lg bg-surface px-4 py-3 shadow-[var(--shadow-border)]"
                >
                  <Check className="mt-0.5 size-4 shrink-0 text-signal" />
                  <span className="text-sm text-fg">{item}</span>
                </li>
              ))}
            </ul>
          </div>
        </section>

        <section className="border-t border-border">
          <div className="mx-auto max-w-6xl px-4 py-16 sm:px-6">
            <p className="text-xs font-medium uppercase tracking-[0.16em] text-subtle">
              Four dimensions
            </p>
            <h2 className="mt-3 max-w-xl font-display text-3xl font-semibold">
              One pass. Runtime, artifact, resource, infrastructure.
            </h2>
            <div className="mt-10 grid gap-4 sm:grid-cols-2">
              {DIMENSIONS.map((d) => {
                const Icon = ICONS[d.id];
                return (
                  <article
                    key={d.id}
                    className="rounded-xl bg-surface p-5 shadow-[var(--shadow-border)]"
                  >
                    <Icon className="size-5 text-accent" />
                    <h3 className="mt-4 font-display text-xl font-semibold">{d.label}</h3>
                    <p className="mt-2 text-sm text-muted">{d.blurb}</p>
                  </article>
                );
              })}
            </div>
          </div>
        </section>

        <section className="border-t border-border">
          <div className="mx-auto max-w-6xl px-4 py-16 sm:px-6">
            <p className="text-xs font-medium uppercase tracking-[0.16em] text-subtle">
              Closed loop
            </p>
            <h2 className="mt-3 font-display text-3xl font-semibold">
              Analyze → profile → transform → rebuild → verify → report
            </h2>
            <ol className="mt-10 grid gap-2 sm:grid-cols-2 lg:grid-cols-4">
              {PIPELINE.map((s, i) => (
                <li
                  key={s.id}
                  className="rounded-lg bg-surface px-4 py-4 shadow-[var(--shadow-border)]"
                >
                  <p className="font-mono text-[10px] text-subtle">
                    {String(i + 1).padStart(2, "0")}
                  </p>
                  <p className="mt-2 font-medium">{s.label}</p>
                  <p className="mt-1 text-xs text-muted">{s.tool}</p>
                </li>
              ))}
            </ol>
          </div>
        </section>

        <section className="border-t border-border">
          <div className="mx-auto grid max-w-6xl gap-10 px-4 py-16 sm:px-6 lg:grid-cols-3">
            <article className="rounded-xl bg-surface p-6 shadow-[var(--shadow-border)] lg:col-span-2">
              <div className="flex items-center gap-2 text-accent">
                <Lock className="size-4" />
                <p className="text-xs font-medium uppercase tracking-[0.16em]">Local-first</p>
              </div>
              <h2 className="mt-4 font-display text-2xl font-semibold">
                Binaries never leave the machine.
              </h2>
              <p className="mt-3 text-sm text-muted">
                The native agent binds 127.0.0.1 and a unix socket. This UI does not upload
                artifacts to a cloud optimizer. Docker/OCI archives can be inspected; they are
                not rewritten without a Dockerfile and a test command.
              </p>
            </article>
            <article className="rounded-xl bg-surface p-6 shadow-[var(--shadow-border)]">
              <p className="text-xs font-medium uppercase tracking-[0.16em] text-subtle">Pricing</p>
              <ul className="mt-4 space-y-3 text-sm">
                <li>
                  <span className="text-fg">Local studio + agent</span>
                  <span className="mt-0.5 block text-muted">Free forever</span>
                </li>
                <li>
                  <span className="text-fg">CI / CD minutes</span>
                  <span className="mt-0.5 block text-muted">Usage, for teams</span>
                </li>
                <li>
                  <span className="text-fg">Optimization report</span>
                  <span className="mt-0.5 block text-muted">Local JSON + checksums</span>
                </li>
              </ul>
            </article>
          </div>
        </section>

        <section className="border-t border-border">
          <div className="mx-auto flex max-w-6xl flex-col items-start justify-between gap-6 px-4 py-16 sm:flex-row sm:items-center sm:px-6">
            <div>
              <h2 className="font-display text-3xl font-semibold">Drop an ELF. Or run the sample.</h2>
              <p className="mt-2 text-muted">Demo workloads are labeled. Live runs need the local agent.</p>
            </div>
            <Button asChild size="lg">
              <Link to="/studio">
                Open studio
                <ArrowRight />
              </Link>
            </Button>
          </div>
        </section>
      </main>
      <Footer />
    </Shell>
  );
}

function HeroProof() {
  return (
    <div className="mt-8 overflow-hidden rounded-2xl bg-surface p-1 shadow-[var(--shadow-border)]">
      <div className="rounded-xl bg-surface-2 px-4 py-3 sm:px-5">
        <p className="font-mono text-[11px] text-subtle">sample · ghcr.io/acme/orders-api:1.8.2</p>
        <div className="mt-3 flex flex-wrap gap-2">
          <Badge variant="warn">{DEMO_LABEL}</Badge>
          <Badge variant="warn">{MODELED_LABEL}</Badge>
          <Badge variant="warn">{NOT_EXECUTED_LABEL}</Badge>
        </div>
        <div className="mt-4 grid grid-cols-2 gap-3 sm:grid-cols-4">
          <ProofStat label="Image" before="812 MB" after="118 MB" delta="−85%" />
          <ProofStat label="Throughput" before="1,840 rps" after="2,112 rps" delta="+15%" />
          <ProofStat label="RSS" before="186 MiB" after="94 MiB" delta="−49%" />
          <ProofStat label="Tests" before="—" after="142/142" delta="PASS" />
        </div>
        <div className="mt-5 h-2 overflow-hidden rounded-full bg-bg">
          <div className="flex h-full">
            <div className="w-[14%] bg-signal" />
            <div className="flex-1 bg-subtle/30" />
          </div>
        </div>
        <div className="mt-2 flex justify-between font-mono text-[10px] text-subtle">
          <span>after 118 MB</span>
          <span>before 812 MB</span>
        </div>
      </div>
    </div>
  );
}

function ProofStat({
  label,
  before,
  after,
  delta,
}: {
  label: string;
  before: string;
  after: string;
  delta: string;
}) {
  return (
    <div>
      <p className="text-[11px] uppercase tracking-wider text-subtle">{label}</p>
      <p className="mt-1 font-mono text-lg tabular-nums text-fg">{after}</p>
      <p className="text-xs text-muted">
        from {before} <span className="text-signal">{delta}</span>
      </p>
    </div>
  );
}
