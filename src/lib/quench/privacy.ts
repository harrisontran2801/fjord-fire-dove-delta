import type { Run } from "./types";

/** Secure-by-default: inspection history is not retained unless the user opts out. */
export const DEFAULT_DELETE_INSPECTION_DATA = true;

export function isInspectionRun(run: Pick<Run, "mode">): boolean {
  return run.mode === "inspect";
}

/**
 * Resolve the privacy flag from persisted local state.
 * Missing or invalid values default to ON so legacy history is treated as private.
 */
export function privacyFlagFromStorage(value: unknown): boolean {
  return typeof value === "boolean" ? value : DEFAULT_DELETE_INSPECTION_DATA;
}

/**
 * Runs that may be written to browser history.
 * When deletion is ON, inspection runs are omitted; demo and native agent runs stay.
 */
export function persistedRuns<T extends Pick<Run, "mode">>(
  runs: readonly T[],
  deleteInspectionData: boolean,
): T[] {
  return deleteInspectionData ? runs.filter((run) => !isInspectionRun(run)) : [...runs];
}

/**
 * Immediate effect of changing the toggle.
 * Enabling deletion removes existing inspection runs from the current session;
 * disabling leaves current runs in place so they can be retained locally.
 */
export function runsAfterPrivacyToggle<T extends Pick<Run, "mode">>(
  runs: readonly T[],
  enabled: boolean,
): T[] {
  return persistedRuns(runs, enabled);
}

/**
 * Rehydrate stored history. A missing flag is treated as ON, which filters
 * legacy inspection runs out of old localStorage snapshots.
 */
export function rehydratePersistedRuns<T extends Pick<Run, "mode">>(
  runs: readonly T[],
  storedDeleteInspectionData?: unknown,
): T[] {
  return persistedRuns(runs, privacyFlagFromStorage(storedDeleteInspectionData));
}
