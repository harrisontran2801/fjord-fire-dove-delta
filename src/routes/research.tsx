import { createFileRoute, Link } from "@tanstack/react-router";
import { ArrowRight } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Footer, Shell } from "@/components/layout/shell";
import { COMPETITORS, DIMENSIONS } from "@/lib/quench/data";
import { cn } from "@/lib/utils";

export const Route = createFileRoute("/research")({ component: ResearchPage });

function ResearchPage() {
  return (
    <Shell>
      <main className="mx-auto max-w-6xl px-4 py-12 sm:px-6">
        <p className="text-xs font-medium uppercase tracking-[0.16em] text-subtle">
          Independent briefing
        </p>
        <h1 className="mt-3 max-w-3xl font-display text-4xl font-semibold">
          Market and feasibility for a local-first, multi-dimension optimizer
        </h1>
        <p className="mt-4 max-w-2xl text-muted">
          Vibe coding accelerated source generation and exploded technical debt: duplicated code,
          fat images, wasted RAM, record cloud bills. The market is full of point tools. It is
          missing a closed local loop.
        </p>

        <section className="mt-12 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {[
            { k: "Fragmented", v: "BOLT, Slim, CAST, UPX each own one slice." },
            { k: "Gap", v: "No analyze → optimize → verify → prove on the laptop." },
            { k: "MVP", v: "Linux ELF + Docker via LLVM, Bloaty, UPX, Zstd, containerd." },
            { k: "Model", v: "Free local. Paid CI and certification. Infra cost ≈ $0." },
          ].map((x) => (
            <article key={x.k} className="rounded-xl bg-surface p-4 shadow-[var(--shadow-border)]">
              <p className="text-xs uppercase tracking-wider text-subtle">{x.k}</p>
              <p className="mt-2 text-sm">{x.v}</p>
            </article>
          ))}
        </section>

        <section className="mt-14">
          <h2 className="font-display text-2xl font-semibold">Market map</h2>
          <ol className="mt-4 grid gap-2 sm:grid-cols-2">
            {[
              "Direct — Slim.ai, Granulate, Opsani",
              "Indirect FinOps — CAST AI, Kubecost, ScaleOps",
              "Open source — LLVM BOLT, UPX, Bloaty, Binaryen",
              "Dev tools — Advisor, Valgrind, perf, eBPF",
              "Enterprise APM — Dynatrace, Datadog, Turbonomic",
              "Cloud advisors — AWS, GCP, Azure recommenders",
              "AI coding — Copilot, Cursor, Claude Code, AwareCompiler",
            ].map((row, i) => (
              <li key={row} className="flex gap-3 rounded-lg bg-surface px-4 py-3 text-sm shadow-[var(--shadow-border)]">
                <span className="font-mono text-subtle">{String(i + 1).padStart(2, "0")}</span>
                {row}
              </li>
            ))}
          </ol>
        </section>

        <section className="mt-14">
          <h2 className="font-display text-2xl font-semibold">Competitor matrix</h2>
          <p className="mt-2 text-sm text-muted">
            Quench is the only row that covers four dimensions, functional verification, and a
            certificate.
          </p>
          <div className="mt-4 overflow-x-auto rounded-xl shadow-[var(--shadow-border)]">
            <table className="min-w-[720px] w-full text-left text-sm">
              <thead className="bg-surface-2 text-xs uppercase tracking-wider text-subtle">
                <tr>
                  <th className="px-3 py-3 font-medium">Product</th>
                  <th className="px-3 py-3 font-medium">Where</th>
                  <th className="px-3 py-3 font-medium">Axes</th>
                  <th className="px-3 py-3 font-medium">Verify</th>
                  <th className="px-3 py-3 font-medium">Cert</th>
                </tr>
              </thead>
              <tbody className="bg-surface">
                {COMPETITORS.map((c) => (
                  <tr key={c.name} className="border-t border-border">
                    <td className="px-3 py-3">
                      <p className={cn("font-medium", c.name === "Quench" && "text-accent")}>{c.name}</p>
                      <p className="text-xs text-subtle">{c.focus}</p>
                    </td>
                    <td className="px-3 py-3 text-muted">{c.localCloud}</td>
                    <td className="px-3 py-3">
                      <div className="flex flex-wrap gap-1">
                        {DIMENSIONS.map((d) => (
                          <span
                            key={d.id}
                            className={cn(
                              "rounded-full px-2 py-0.5 text-[10px]",
                              c.dimensions.includes(d.id)
                                ? "bg-signal/15 text-signal"
                                : "bg-surface-2 text-subtle",
                            )}
                          >
                            {d.label}
                          </span>
                        ))}
                      </div>
                    </td>
                    <td className="px-3 py-3">{c.verify ? "Yes" : "—"}</td>
                    <td className="px-3 py-3">{c.cert ? "Yes" : "—"}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        <section className="mt-14 grid gap-4 lg:grid-cols-3">
          {[
            {
              t: "Equivalence",
              p: "BOLT and UPX assume transforms do not break logic. Edge-case crashes and alignment bugs exist. A local sandbox must run the project test suite before a certificate is issued.",
            },
            {
              t: "Pareto conflict",
              p: "UPX shrinks disk and can raise cold start plus RSS via the unpack stub. The product must sit on a size↔speed frontier, not blindly compress.",
            },
            {
              t: "Manifest gap",
              p: "A 30% RSS win is worthless on the bill if Helm still requests 512Mi. Artifact and runtime gains have to rewrite resources.requests.",
            },
          ].map((x) => (
            <article key={x.t} className="rounded-xl bg-surface p-5 shadow-[var(--shadow-border)]">
              <h3 className="font-display text-xl font-semibold">{x.t}</h3>
              <p className="mt-2 text-sm text-muted">{x.p}</p>
            </article>
          ))}
        </section>

        <section className="mt-14">
          <h2 className="font-display text-2xl font-semibold">Who pays</h2>
          <div className="mt-4 grid gap-3 sm:grid-cols-2">
            {[
              {
                t: "Platform / DevOps",
                p: "Registry cost and 1–3 GB images that miss HPA SLAs. Slimming has been shown to speed deploys ~5×.",
              },
              {
                t: "Indie / vibe coders",
                p: "OOM-kills on small VPS. No LLVM expertise. They need a drag-and-drop local tool.",
              },
              {
                t: "HFT / games",
                p: "Microsecond p99 and L1 i-cache layout. BOLT-class 10–15% CPU is the pitch.",
              },
              {
                t: "ESG / FinOps",
                p: "Scope 3 software carbon. GSF SCI certificates are the enterprise wedge.",
              },
            ].map((x) => (
              <article key={x.t} className="rounded-xl bg-surface p-5 shadow-[var(--shadow-border)]">
                <h3 className="font-medium">{x.t}</h3>
                <p className="mt-2 text-sm text-muted">{x.p}</p>
              </article>
            ))}
          </div>
        </section>

        <section className="mt-14">
          <h2 className="font-display text-2xl font-semibold">Moat, in order</h2>
          <ol className="mt-4 space-y-2">
            {[
              "C1 — GUI wrapper of UPX/BOLT/strip. Copied in a weekend.",
              "C2 — Multi-objective Pareto transform. Medium.",
              "C3 — Community profile data flywheel. Strong.",
              "C4 — Cost & energy certification standard. Strongest.",
            ].map((row) => (
              <li key={row} className="rounded-lg bg-surface px-4 py-3 text-sm shadow-[var(--shadow-border)]">
                {row}
              </li>
            ))}
          </ol>
        </section>

        <section className="mt-14 rounded-2xl bg-surface p-6 shadow-[var(--shadow-border)] sm:p-8">
          <Badge variant="accent">Positioning</Badge>
          <h2 className="mt-4 font-display text-2xl font-semibold">
            The deterministic layer after AI-generated software
          </h2>
          <p className="mt-3 max-w-2xl text-sm text-muted">
            AI is the generator. Quench is the verifier and optimizer. Linux + Docker covers the
            majority of backend and cloud-native artifacts. A solo founder can ship an MVP in a
            quarter because the heavy compute stays on the user's machine.
          </p>
          <Button asChild className="mt-6">
            <Link to="/studio">
              Open the studio
              <ArrowRight className="size-4" />
            </Link>
          </Button>
        </section>
      </main>
      <Footer />
    </Shell>
  );
}
