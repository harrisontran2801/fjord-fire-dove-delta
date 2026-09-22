import { create } from "zustand";
import { persist } from "zustand/middleware";
import { createAgentRun, createRun, pipelineCompleteMs, revealedStage, runModeFor, stagesFor } from "./engine";
import type { Artifact, NativeReport, ParetoPref, Run } from "./types";

function normalizeRun(run: Run): Run {
  const mode = run.mode ?? runModeFor(run.artifact);
  const modeled = mode === "demo" ? (run.result?.modeled ?? true) : false;
  return {
    ...run,
    mode,
    result: run.result ? { ...run.result, modeled } : run.result,
  };
}

interface QuenchState {
  runs: Run[];
  preference: ParetoPref;
  hydrated: boolean;
  setHydrated: () => void;
  setPreference: (p: ParetoPref) => void;
  startRun: (artifact: Artifact, preference?: ParetoPref) => Run;
  startAgentRun: (args: {
    artifact: Artifact;
    report: NativeReport & Record<string, unknown>;
    logs: string[];
    transformsApplied: { tool: string; status: string; detail: string }[];
    transformsFailed: { tool: string; status: string; detail: string }[];
    runId?: string;
    agentStatus?: string;
  }) => Run;
  markCompleteIfDue: (id: string) => void;
  setRunPreference: (id: string, p: ParetoPref) => void;
  removeRun: (id: string) => void;
  getRun: (id: string) => Run | undefined;
}

export const useQuenchStore = create<QuenchState>()(
  persist(
    (set, get) => ({
      runs: [],
      preference: "balanced",
      hydrated: false,
      setHydrated: () => set({ hydrated: true }),
      setPreference: (p) => set({ preference: p }),
      startRun: (artifact, preference) => {
        const pref = preference ?? get().preference;
        const run = createRun(artifact, pref);
        set((s) => ({ runs: [run, ...s.runs.filter((r) => r.id !== run.id)].slice(0, 24) }));
        return run;
      },
      startAgentRun: (args) => {
        const run = createAgentRun(args);
        set((s) => ({ runs: [run, ...s.runs.filter((r) => r.id !== run.id)].slice(0, 24) }));
        return run;
      },
      markCompleteIfDue: (id) => {
        const run = get().runs.find((r) => r.id === id);
        if (!run || run.status !== "running") return;
        if (run.mode === "agent") return;
        if (revealedStage(run) >= stagesFor(run.mode).length) {
          set((s) => ({
            runs: s.runs.map((r) => (r.id === id ? { ...r, status: "complete" as const } : r)),
          }));
        }
      },
      setRunPreference: (id, p) =>
        set((s) => ({
          runs: s.runs.map((r) => {
            if (r.id !== id) return r;
            if (r.mode !== "demo") return r;
            return { ...r, preference: p };
          }),
        })),
      removeRun: (id) => set((s) => ({ runs: s.runs.filter((r) => r.id !== id) })),
      getRun: (id) => get().runs.find((r) => r.id === id),
    }),
    {
      name: "quench-studio-v3",
      partialize: (s) => ({ runs: s.runs, preference: s.preference }),
      onRehydrateStorage: () => (state) => {
        const now = Date.now();
        if (!state) return;
        const runs = (state.runs ?? []).map((r) => {
          const run = normalizeRun(r);
          if (run.mode === "agent") return run;
          return run.status === "running" && now - run.startedAt >= pipelineCompleteMs(run.mode)
            ? { ...run, status: "complete" as const }
            : run;
        });
        useQuenchStore.setState({ runs, hydrated: true });
      },
    },
  ),
);
