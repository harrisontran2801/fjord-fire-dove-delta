#!/bin/sh
set -eu

# Repository root is the directory that contains this script, not a hard-coded
# /workspace path. QUENCH_WORKSPACE still wins when the caller sets it.
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cd "$ROOT"

if [ "${1:-}" = "--print-env" ]; then
  sh "$ROOT/scripts/start-agent.sh" --print-env
  exit 0
fi

# :8081 is QA-only — a revive must never inherit a stale built-output preview.
node scripts/preview.mjs stop || true

export QUENCH_WORKSPACE="${QUENCH_WORKSPACE:-$ROOT}"
export QUENCH_SAMPLE_CONFIG="${QUENCH_SAMPLE_CONFIG:-samples/match-engine/quench.yaml}"

# Native agent is optional. Missing Rust must not take down the web app.
# The UI probes /api/agent and shows an unavailable state when this is down.
# start-agent.sh backgrounds the binary and polls until it is ready (or gives up).
sh "$ROOT/scripts/start-agent.sh" >>/tmp/quench-agent-startup.log 2>&1 || true

if curl -sf -o /dev/null --max-time 2 http://127.0.0.1:8080/; then
  exit 0
fi

npm run dev >>/tmp/app-startup.log 2>&1 &
DEV_PID=$!

# Do not return success if the web process died before Vite accepted traffic.
i=0
while [ "$i" -lt 90 ]; do
  if curl -sf -o /dev/null --max-time 2 http://127.0.0.1:8080/; then
    exit 0
  fi
  if ! kill -0 "$DEV_PID" 2>/dev/null; then
    echo "web process exited before Vite was ready (pid $DEV_PID); see /tmp/app-startup.log" >&2
    exit 1
  fi
  i=$((i + 1))
  sleep 0.5
done

echo "timed out waiting for Vite; web process still running (pid $DEV_PID)" >&2
exit 0
