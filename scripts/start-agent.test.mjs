import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = join(SCRIPT_DIR, "..");
const START_AGENT = join(SCRIPT_DIR, "start-agent.sh");
const STARTUP = join(REPO_ROOT, "startup.sh");

function parseEnv(stdout) {
  const out = {};
  for (const line of stdout.split("\n")) {
    const idx = line.indexOf("=");
    if (idx === -1) continue;
    out[line.slice(0, idx)] = line.slice(idx + 1);
  }
  return out;
}

function cleanEnv(extra = {}) {
  const env = { ...process.env };
  delete env.QUENCH_WORKSPACE;
  delete env.QUENCH_AGENT_BIN;
  delete env.QUENCH_AGENT_SOCKET;
  delete env.QUENCH_SAMPLE_CONFIG;
  delete env.QUENCH_AGENT_BIND;
  return { ...env, ...extra };
}

test("start-agent.sh and startup.sh do not hard-code /workspace as the repo root", () => {
  const agentSrc = readFileSync(START_AGENT, "utf8");
  const startupSrc = readFileSync(STARTUP, "utf8");
  for (const [name, src] of [
    ["scripts/start-agent.sh", agentSrc],
    ["startup.sh", startupSrc],
  ]) {
    assert.equal(/\ncd \/workspace\n/.test(src), false, `${name} still cds to /workspace`);
    assert.equal(src.includes("/workspace/agent/target"), false, `${name} hard-codes agent bin`);
    assert.equal(src.includes("/workspace/agent/Cargo.toml"), false, `${name} hard-codes Cargo.toml`);
    assert.equal(src.includes("/workspace/scripts/start-agent.sh"), false, `${name} hard-codes start-agent path`);
    assert.match(src, /QUENCH_WORKSPACE/);
    assert.match(src, /QUENCH_SAMPLE_CONFIG/);
    assert.match(src, /dirname/);
  }
  assert.match(agentSrc, /QUENCH_AGENT_BIN/);
  assert.match(agentSrc, /QUENCH_AGENT_SOCKET/);
});

test("start-agent.sh --print-env resolves a repository path other than /workspace", async () => {
  const root = mkdtempSync(join(tmpdir(), "quench-other-root-"));
  assert.notEqual(root, "/workspace");
  mkdirSync(join(root, "scripts"), { recursive: true });
  const copied = join(root, "scripts/start-agent.sh");
  copyFileSync(START_AGENT, copied);
  chmodSync(copied, 0o755);
  const { stdout } = await execFileAsync("sh", [copied, "--print-env"], { env: cleanEnv() });
  const env = parseEnv(stdout);
  assert.equal(env.QUENCH_ROOT, root);
  assert.equal(env.QUENCH_WORKSPACE, root);
  assert.equal(env.QUENCH_AGENT_BIN, join(root, "agent/target/release/quench-agent"));
  assert.equal(env.QUENCH_AGENT_SOCKET, "/tmp/quench-agent.sock");
  assert.equal(env.QUENCH_SAMPLE_CONFIG, "samples/match-engine/quench.yaml");
  assert.equal(env.QUENCH_AGENT_BIND, "127.0.0.1:4783");
});

test("start-agent.sh --print-env respects QUENCH_* overrides", async () => {
  const root = mkdtempSync(join(tmpdir(), "quench-override-root-"));
  mkdirSync(join(root, "scripts"), { recursive: true });
  const copied = join(root, "scripts/start-agent.sh");
  copyFileSync(START_AGENT, copied);
  chmodSync(copied, 0o755);
  const { stdout } = await execFileAsync("sh", [copied, "--print-env"], {
    env: cleanEnv({
      QUENCH_WORKSPACE: "/tmp/custom-ws",
      QUENCH_AGENT_BIN: "/tmp/custom-bin",
      QUENCH_AGENT_SOCKET: "/tmp/custom-agent.sock",
      QUENCH_SAMPLE_CONFIG: "samples/custom/quench.yaml",
      QUENCH_AGENT_BIND: "127.0.0.1:4999",
    }),
  });
  const env = parseEnv(stdout);
  assert.equal(env.QUENCH_ROOT, root);
  assert.equal(env.QUENCH_WORKSPACE, "/tmp/custom-ws");
  assert.equal(env.QUENCH_AGENT_BIN, "/tmp/custom-bin");
  assert.equal(env.QUENCH_AGENT_SOCKET, "/tmp/custom-agent.sock");
  assert.equal(env.QUENCH_SAMPLE_CONFIG, "samples/custom/quench.yaml");
  assert.equal(env.QUENCH_AGENT_BIND, "127.0.0.1:4999");
});
