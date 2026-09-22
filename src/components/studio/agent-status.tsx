import { Badge } from "@/components/ui/badge";
import { AGENT_UNAVAILABLE_HINT, type AgentStatus } from "@/lib/quench/agent";
import type { Run } from "@/lib/quench/types";
import { cn } from "@/lib/utils";

export function agentStateLabel(agent: AgentStatus | null, run?: Run): string {
  if (run?.status === "verification_failed") return "Verification failed";
  if (run?.mode === "agent" && run.status === "failed") return "Failed";
  if (run?.mode === "agent" && run.status === "complete") return "Optimization complete";
  if (run?.mode === "agent" && run.status === "running") return "Optimizing";
  if (run?.mode === "inspect") return run.status === "running" ? "Inspecting" : "Inspection only";
  if (run?.mode === "demo") return "Demo mode";
  if (!agent || !agent.connected) return "Local agent not connected";
  return "Agent connected";
}

export function AgentBadge({
  agent,
  run,
}: {
  agent: AgentStatus | null;
  run?: Run;
}) {
  const label = agentStateLabel(agent, run);
  const variant =
    label === "Verification failed" || label === "Failed"
      ? "danger"
      : label === "Local agent not connected"
        ? "warn"
        : label === "Demo mode" || label === "Inspection only"
          ? "warn"
          : label === "Agent connected" || label === "Optimization complete"
            ? "signal"
            : "accent";
  return <Badge variant={variant}>{label}</Badge>;
}

export function AgentBanner({ agent }: { agent: AgentStatus | null }) {
  const connected = Boolean(agent?.connected);
  return (
    <div
      className={cn(
        "rounded-xl px-4 py-3 text-sm shadow-[var(--shadow-border)]",
        connected ? "bg-surface" : "bg-surface-2",
      )}
    >
      <div className="flex flex-wrap items-center gap-2">
        <AgentBadge agent={agent} />
        {connected ? (
          <p className="text-muted">
            Native agent {agent?.version ?? ""} · loopback only · binaries stay on this machine
          </p>
        ) : (
          <p className="text-muted">
            Agent unavailable. Studio will not pretend a pipeline is running. {AGENT_UNAVAILABLE_HINT}
          </p>
        )}
      </div>
      {!connected ? (
        <p className="mt-2 font-mono text-xs text-subtle">{AGENT_UNAVAILABLE_HINT}</p>
      ) : (
        <DoctorStrip doctor={agent?.doctor ?? null} />
      )}
    </div>
  );
}

function DoctorStrip({ doctor }: { doctor: AgentStatus["doctor"] }) {
  if (!doctor) return null;
  const missing = doctor.checks.filter((c) => c.status === "unavailable");
  if (missing.length === 0) {
    return <p className="mt-2 text-xs text-signal">Doctor: required host checks passed.</p>;
  }
  return (
    <p className="mt-2 text-xs text-muted">
      Unavailable: {missing.map((c) => c.name).join(" · ")}. Missing tools are reported, not faked.
    </p>
  );
}
