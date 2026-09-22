import assert from "node:assert/strict";
import { test } from "node:test";
import {
  DEFAULT_DELETE_INSPECTION_DATA,
  isInspectionRun,
  persistedRuns,
  privacyFlagFromStorage,
  rehydratePersistedRuns,
  runsAfterPrivacyToggle,
} from "./privacy.ts";
import type { Run } from "./types.ts";

function run(mode: Run["mode"], id: string): Run {
  return { id, mode } as Run;
}

test("privacy defaults to deleting inspection data", () => {
  assert.equal(DEFAULT_DELETE_INSPECTION_DATA, true);
  assert.equal(privacyFlagFromStorage(undefined), true);
  assert.equal(privacyFlagFromStorage(null), true);
  assert.equal(privacyFlagFromStorage("false"), true);
});

test("stored boolean flags are honored", () => {
  assert.equal(privacyFlagFromStorage(true), true);
  assert.equal(privacyFlagFromStorage(false), false);
});

test("isInspectionRun only matches inspect mode", () => {
  assert.equal(isInspectionRun(run("inspect", "i")), true);
  assert.equal(isInspectionRun(run("demo", "d")), false);
  assert.equal(isInspectionRun(run("agent", "a")), false);
});

test("OFF retains inspection history locally", () => {
  const runs = [run("inspect", "inspect-1"), run("demo", "demo-1"), run("agent", "agent-1")];
  assert.deepEqual(persistedRuns(runs, false), runs);
});

test("ON omits inspection runs from persisted history", () => {
  const inspect = run("inspect", "inspect-1");
  const demo = run("demo", "demo-1");
  const agent = run("agent", "agent-1");
  assert.deepEqual(persistedRuns([inspect, demo, agent], true), [demo, agent]);
});

test("enabling the setting deletes existing inspection runs immediately", () => {
  const inspectA = run("inspect", "inspect-a");
  const inspectB = run("inspect", "inspect-b");
  const demo = run("demo", "demo-1");
  const current = [inspectA, demo, inspectB];
  assert.deepEqual(runsAfterPrivacyToggle(current, true), [demo]);
});

test("disabling the setting keeps current inspection runs", () => {
  const inspect = run("inspect", "inspect-1");
  const demo = run("demo", "demo-1");
  const current = [inspect, demo];
  assert.deepEqual(runsAfterPrivacyToggle(current, false), current);
});

test("rehydration with the flag ON filters legacy inspection runs", () => {
  const inspect = run("inspect", "legacy-inspect");
  const demo = run("demo", "demo-1");
  assert.deepEqual(rehydratePersistedRuns([inspect, demo], true), [demo]);
});

test("rehydration with the flag OFF keeps inspection history", () => {
  const inspect = run("inspect", "kept-inspect");
  const demo = run("demo", "demo-1");
  const stored = [inspect, demo];
  assert.deepEqual(rehydratePersistedRuns(stored, false), stored);
});

test("rehydrating old local state without a flag defaults ON and drops inspect runs", () => {
  const inspect = run("inspect", "legacy-inspect");
  const demo = run("demo", "demo-1");
  const agent = run("agent", "agent-1");
  assert.deepEqual(rehydratePersistedRuns([inspect, demo, agent], undefined), [demo, agent]);
  assert.deepEqual(rehydratePersistedRuns([inspect, demo], null), [demo]);
});

test("filtering does not mutate the original runs array", () => {
  const runs = [run("inspect", "inspect-1"), run("demo", "demo-1")];
  const snapshot = [...runs];
  persistedRuns(runs, true);
  runsAfterPrivacyToggle(runs, true);
  rehydratePersistedRuns(runs, undefined);
  assert.deepEqual(runs, snapshot);
});
