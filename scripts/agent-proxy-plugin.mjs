import http from "node:http";

const DEFAULT_BIND = "127.0.0.1:4783";
const AGENT_BIND = process.env.QUENCH_AGENT_BIND ?? `127.0.0.1:${process.env.QUENCH_AGENT_PORT ?? 4783}`;
const AGENT_SOCKET = process.env.QUENCH_AGENT_SOCKET ?? "/tmp/quench-agent.sock";

function parseBind(bind) {
  const value = String(bind).trim() || DEFAULT_BIND;
  if (value.startsWith("[")) {
    const close = value.indexOf("]:");
    if (close > 0) return { host: value.slice(1, close), port: Number(value.slice(close + 2)) };
  }
  const split = value.lastIndexOf(":");
  if (split > 0) return { host: value.slice(0, split), port: Number(value.slice(split + 1)) };
  return { host: value, port: 4783 };
}

const { host: AGENT_HOST, port: AGENT_PORT } = parseBind(AGENT_BIND);
const AGENT_HOST_HEADER = AGENT_HOST.includes(":") ? `[${AGENT_HOST}]:${AGENT_PORT}` : `${AGENT_HOST}:${AGENT_PORT}`;

function hopHeaders(req, bodyLength) {
  const headers = { ...req.headers, host: AGENT_HOST_HEADER };
  // Local unix/TCP hop: do not forward the browser Origin so a preview host
  // cannot be confused with a CSRF from an unknown site hitting 127.0.0.1:4783.
  delete headers.origin;
  delete headers.referer;
  delete headers.Origin;
  delete headers.Referer;
  delete headers["transfer-encoding"];
  if (bodyLength > 0 || headers["content-length"] !== undefined) {
    headers["content-length"] = String(bodyLength);
  }
  return headers;
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(Buffer.from(chunk)));
    req.once("end", () => resolve(Buffer.concat(chunks)));
    req.once("error", reject);
    req.once("aborted", () => reject(new Error("request aborted")));
  });
}

function requestUpstream(options, req, body) {
  return new Promise((resolve, reject) => {
    const upstream = http.request(
      {
        ...options,
        method: req.method,
        headers: hopHeaders(req, body.length),
      },
      resolve,
    );
    upstream.on("error", reject);
    upstream.end(body);
  });
}

async function forward(req, res) {
  const url = req.url ?? "/";
  const targetPath = url.replace(/^\/api\/agent/, "") || "/";
  let body;
  try {
    body = await readBody(req);
  } catch {
    res.statusCode = 400;
    res.end(JSON.stringify({ ok: false, error: "Could not read agent request body" }));
    return;
  }
  const path = targetPath.startsWith("/") ? targetPath : `/${targetPath}`;
  const tryUnix = () => requestUpstream({ socketPath: AGENT_SOCKET, path }, req, body);
  const tryTcp = () => requestUpstream({ host: AGENT_HOST, port: AGENT_PORT, path }, req, body);

  const pipeRes = (upstream) => {
    res.statusCode = upstream.statusCode ?? 502;
    for (const [k, v] of Object.entries(upstream.headers)) {
      if (v === undefined) continue;
      if (String(k).toLowerCase().startsWith("access-control-")) continue;
      res.setHeader(k, v);
    }
    upstream.pipe(res);
  };

  try {
    pipeRes(await tryUnix());
  } catch {
    try {
      pipeRes(await tryTcp());
    } catch {
      res.statusCode = 503;
      res.setHeader("content-type", "application/json");
      res.end(
        JSON.stringify({
          ok: false,
          error: "Local agent not connected",
          hint: `Start the agent with npm run agent:serve (loopback ${AGENT_HOST_HEADER}, unix ${AGENT_SOCKET}). The web app will stay in Demo / Inspection mode until it is up. Binaries stay on this machine.`,
        }),
      );
    }
  }
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
