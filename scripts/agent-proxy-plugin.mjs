import http from "node:http";

const AGENT_HOST = "127.0.0.1";
const AGENT_PORT = Number(process.env.QUENCH_AGENT_PORT ?? 4783);
const AGENT_SOCKET = process.env.QUENCH_AGENT_SOCKET ?? "/tmp/quench-agent.sock";

function hopHeaders(req) {
  const headers = { ...req.headers, host: `${AGENT_HOST}:${AGENT_PORT}` };
  // Local unix/TCP hop: do not forward the browser Origin so a preview host
  // cannot be confused with a CSRF from an unknown site hitting 127.0.0.1:4783.
  delete headers.origin;
  delete headers.referer;
  delete headers.Origin;
  delete headers.Referer;
  return headers;
}

function forward(req, res) {
  const url = req.url ?? "/";
  const targetPath = url.replace(/^\/api\/agent/, "") || "/";
  const tryUnix = () =>
    new Promise((resolve, reject) => {
      const p = http.request(
        {
          socketPath: AGENT_SOCKET,
          path: targetPath.startsWith("/") ? targetPath : `/${targetPath}`,
          method: req.method,
          headers: hopHeaders(req),
        },
        resolve,
      );
      p.on("error", reject);
      req.pipe(p);
    });
  const tryTcp = () =>
    new Promise((resolve, reject) => {
      const p = http.request(
        {
          host: AGENT_HOST,
          port: AGENT_PORT,
          path: targetPath.startsWith("/") ? targetPath : `/${targetPath}`,
          method: req.method,
          headers: hopHeaders(req),
        },
        resolve,
      );
      p.on("error", reject);
      req.pipe(p);
    });

  const pipeRes = (upstream) => {
    res.statusCode = upstream.statusCode ?? 502;
    for (const [k, v] of Object.entries(upstream.headers)) {
      if (v === undefined) continue;
      if (String(k).toLowerCase().startsWith("access-control-")) continue;
      res.setHeader(k, v);
    }
    upstream.pipe(res);
  };

  tryUnix()
    .then(pipeRes)
    .catch(() =>
      tryTcp()
        .then(pipeRes)
        .catch(() => {
          res.statusCode = 503;
          res.setHeader("content-type", "application/json");
          res.end(
            JSON.stringify({
              ok: false,
              error: "Local agent not connected",
              hint: "Start the agent with npm run agent:serve (loopback 127.0.0.1:4783, unix /tmp/quench-agent.sock). The web app will stay in Demo / Inspection mode until it is up. Binaries stay on this machine.",
            }),
          );
        }),
    );
}

export function agentProxyPlugin() {
  const attach = (server) => {
    server.middlewares.use((req, res, next) => {
      const pathOnly = (req.url ?? "").split("?", 1)[0] ?? "";
      if (!pathOnly.startsWith("/api/agent")) {
        next();
        return;
      }
      forward(req, res);
    });
  };
  return {
    name: "quench-agent-proxy",
    configureServer(server) {
      attach(server);
    },
    configurePreviewServer(server) {
      attach(server);
    },
  };
}
