#!/bin/sh
set -eu
cd /workspace
# :8081 is QA-only — a revive must never inherit a stale built-output preview.
node scripts/preview.mjs stop || true

export QUENCH_WORKSPACE="${QUENCH_WORKSPACE:-/workspace}"
export QUENCH_SAMPLE_CONFIG="${QUENCH_SAMPLE_CONFIG:-samples/match-engine/quench.yaml}"

# Native agent is optional. Missing Rust must not take down the web app.
# The UI probes /api/agent and shows an unavailable state when this is down.
sh /workspace/scripts/start-agent.sh >>/tmp/quench-agent-startup.log 2>&1 || true

if curl -sf -o /dev/null --max-time 2 http://127.0.0.1:8080/; then
  exit 0
fi
npm run dev >>/tmp/app-startup.log 2>&1 &
