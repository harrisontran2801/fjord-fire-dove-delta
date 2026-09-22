#!/bin/sh
# Build (if needed) and serve the native quench-agent on loopback.
# Missing Rust is not a failure of the web app — Studio shows an unavailable state.
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/.." && pwd)

export QUENCH_WORKSPACE="${QUENCH_WORKSPACE:-$ROOT}"
export QUENCH_SAMPLE_CONFIG="${QUENCH_SAMPLE_CONFIG:-samples/match-engine/quench.yaml}"
BIND="${QUENCH_AGENT_BIND:-127.0.0.1:4783}"
SOCK="${QUENCH_AGENT_SOCKET:-/tmp/quench-agent.sock}"
BIN="${QUENCH_AGENT_BIN:-$ROOT/agent/target/release/quench-agent}"

print_env() {
  printf 'QUENCH_ROOT=%s\n' "$ROOT"
  printf 'QUENCH_WORKSPACE=%s\n' "$QUENCH_WORKSPACE"
  printf 'QUENCH_AGENT_BIN=%s\n' "$BIN"
  printf 'QUENCH_AGENT_SOCKET=%s\n' "$SOCK"
  printf 'QUENCH_SAMPLE_CONFIG=%s\n' "$QUENCH_SAMPLE_CONFIG"
  printf 'QUENCH_AGENT_BIND=%s\n' "$BIND"
}

if [ "${1:-}" = "--print-env" ]; then
  print_env
  exit 0
fi

case "$BIND" in
  '127.0.0.1:'*|'localhost:'*|'[::1]:'*) ;;
  *)
    echo "refusing to bind quench-agent on $BIND (loopback only)" >&2
    exit 1
    ;;
esac

status_url() {
  echo "http://${BIND}/v1/status"
}

agent_ready() {
  curl -sf -o /dev/null --max-time 1 "$(status_url)" 2>/dev/null
}

if agent_ready; then
  echo "quench-agent already running on $BIND"
  chmod 600 "$SOCK" 2>/dev/null || true
  exit 0
fi

if [ ! -x "$BIN" ]; then
  if command -v cargo >/dev/null 2>&1; then
    echo "building quench-agent (release)…"
    cargo build --release --manifest-path "$ROOT/agent/Cargo.toml"
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
AGENT_PID=$!

n=0
while [ "$n" -lt 25 ]; do
  if agent_ready; then
    chmod 600 "$SOCK" 2>/dev/null || true
    echo "started quench-agent on $BIND unix:$SOCK (workspace $QUENCH_WORKSPACE pid $AGENT_PID)"
    exit 0
  fi
  if ! kill -0 "$AGENT_PID" 2>/dev/null; then
    echo "quench-agent exited before becoming ready; Studio stays in Demo / Inspection mode. See /tmp/quench-agent.log"
    exit 0
  fi
  n=$((n + 1))
  sleep 0.2
done

echo "quench-agent did not become ready in time; Studio stays in Demo / Inspection mode. pid $AGENT_PID"
exit 0
