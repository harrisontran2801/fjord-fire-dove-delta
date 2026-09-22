#!/bin/sh
# Build (if needed) and serve the native quench-agent on loopback.
# Missing Rust is not a failure of the web app — Studio shows an unavailable state.
set -eu
cd /workspace

export QUENCH_WORKSPACE="${QUENCH_WORKSPACE:-/workspace}"
export QUENCH_SAMPLE_CONFIG="${QUENCH_SAMPLE_CONFIG:-samples/match-engine/quench.yaml}"
BIND="${QUENCH_AGENT_BIND:-127.0.0.1:4783}"
SOCK="${QUENCH_AGENT_SOCKET:-/tmp/quench-agent.sock}"
BIN="${QUENCH_AGENT_BIN:-/workspace/agent/target/release/quench-agent}"

case "$BIND" in
  127.0.0.1:*|localhost:*|[::1]:*) ;;
  *)
    echo "refusing to bind quench-agent on $BIND (loopback only)" >&2
    exit 1
    ;;
esac

if curl -sf -o /dev/null --max-time 1 "http://127.0.0.1:4783/v1/status" 2>/dev/null; then
  echo "quench-agent already running on 127.0.0.1:4783"
  exit 0
fi

if [ ! -x "$BIN" ]; then
  if command -v cargo >/dev/null 2>&1; then
    echo "building quench-agent (release)…"
    cargo build --release --manifest-path /workspace/agent/Cargo.toml
  else
    echo "quench-agent unavailable: cargo/rustc not found. Studio stays in Demo / Inspection mode."
    exit 0
  fi
fi

if [ ! -x "$BIN" ]; then
  echo "quench-agent unavailable: $BIN was not produced."
  exit 0
fi

mkdir -p /tmp
nohup "$BIN" serve --bind "$BIND" --socket "$SOCK" >>/tmp/quench-agent.log 2>&1 &
echo "started quench-agent on $BIND unix:$SOCK (workspace $QUENCH_WORKSPACE)"
