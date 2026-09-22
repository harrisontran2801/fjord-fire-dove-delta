import assert from "node:assert/strict";
import { test } from "node:test";
import { DEFAULT_SAMPLE_CONFIG, nativeSampleConfig, type AgentStatus } from "./agent.ts";

test("native sample config is relative, not a hard-coded /workspace path", () => {
  assert.equal(DEFAULT_SAMPLE_CONFIG, "samples/match-engine/quench.yaml");
  assert.ok(!DEFAULT_SAMPLE_CONFIG.startsWith("/workspace"));
  assert.equal(nativeSampleConfig(null), DEFAULT_SAMPLE_CONFIG);
  const connected: AgentStatus = {
    connected: true,
    via: "proxy",
    sampleConfig: "samples/match-engine/quench.yaml",
    sampleConfigPath: "/tmp/project/samples/match-engine/quench.yaml",
  };
  assert.equal(nativeSampleConfig(connected), "samples/match-engine/quench.yaml");
  const hardcoded: AgentStatus = {
    connected: true,
    via: "proxy",
    sampleConfig: "/workspace/samples/match-engine/quench.yaml",
  };
  assert.equal(nativeSampleConfig(hardcoded), DEFAULT_SAMPLE_CONFIG);
});
