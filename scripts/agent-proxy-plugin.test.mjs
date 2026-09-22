import assert from "node:assert/strict";
import { createServer } from "node:http";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

function listen(server) {
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server.address().port)));
}

function close(server) {
  return new Promise((resolve, reject) => server.close((err) => (err ? reject(err) : resolve())));
}

test("replays a POST body when Unix-socket proxy access falls back to TCP", async () => {
  const body = JSON.stringify({ configPath: "samples/match-engine/quench.yaml" });
  let received = "";
  const upstream = createServer((req, res) => {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(chunk));
    req.on("end", () => {
      received = Buffer.concat(chunks).toString("utf8");
      res.writeHead(200, { "content-type": "application/json" });
      res.end(JSON.stringify({ ok: true }));
    });
  });
  const upstreamPort = await listen(upstream);
  const missingSocket = join(mkdtempSync(join(tmpdir(), "quench-proxy-")), "missing.sock");
  process.env.QUENCH_AGENT_BIND = `127.0.0.1:${upstreamPort}`;
  process.env.QUENCH_AGENT_SOCKET = missingSocket;
  const { agentProxyPlugin } = await import(`./agent-proxy-plugin.mjs?test=${upstreamPort}`);
  let middleware;
  agentProxyPlugin().configureServer({ middlewares: { use(fn) { middleware = fn; } } });
  const proxy = createServer((req, res) => middleware(req, res, () => res.end("next")));
  const proxyPort = await listen(proxy);
  try {
    const response = await fetch(`http://127.0.0.1:${proxyPort}/api/agent/v1/optimize`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body,
    });
    assert.equal(response.status, 200);
    assert.deepEqual(await response.json(), { ok: true });
    assert.equal(received, body);
  } finally {
    await close(proxy);
    await close(upstream);
    delete process.env.QUENCH_AGENT_BIND;
    delete process.env.QUENCH_AGENT_SOCKET;
  }
});
