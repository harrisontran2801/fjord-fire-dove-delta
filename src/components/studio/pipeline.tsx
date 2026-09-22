import { Check } from "lucide-react";
import { stagesFor, stageProgress } from "@/lib/quench/engine";
import type { Run } from "@/lib/quench/types";
import { cn } from "@/lib/utils";

export function Pipeline({ run, now }: { run: Run; now: number }) {
  const { index, frac } = stageProgress(run, now);
  const stages = stagesFor(run.mode);
  return (
    <ol className="grid grid-cols-4 gap-2 lg:grid-cols-8">
      {stages.map((stage, i) => {
        const done = i < index || (index >= stages.length && run.status === "complete");
        const live = i === index && index < stages.length;
        return (
          <li
            key={stage.id}
            className={cn(
              "rounded-lg bg-surface px-2.5 py-2 shadow-[var(--shadow-border)]",
              live && "shadow-[var(--shadow-border-hover)]",
            )}
          >
            <div className="mb-1.5 flex items-center justify-between gap-1">
              <span className="font-mono text-[10px] text-subtle">{String(i + 1).padStart(2, "0")}</span>
              {done ? (
                <Check className="size-3.5 text-signal" strokeWidth={2.2} />
              ) : live ? (
                <span className="size-1.5 rounded-full bg-accent pipeline-live" />
              ) : (
                <span className="size-1.5 rounded-full bg-border-strong" />
              )}
            </div>
            <p className={cn("text-xs font-medium", done || live ? "text-fg" : "text-muted")}>
              {stage.label}
            </p>
            <p className="mt-0.5 truncate text-[10px] text-subtle">{stage.tool}</p>
            <div className="mt-2 h-0.5 overflow-hidden rounded-full bg-surface-2">
              <div
                className="h-full bg-accent transition-[width] duration-200 ease-out"
                style={{ width: done ? "100%" : live ? `${Math.round(frac * 100)}%` : "0%" }}
              />
            </div>
          </li>
        );
      })}
    </ol>
  );
}
