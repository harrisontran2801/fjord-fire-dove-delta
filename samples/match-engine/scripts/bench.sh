#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
BIN="${QUENCH_BINARY:-$ROOT/target/release/match-engine}"
if [ ! -x "$BIN" ]; then
  echo "benchmark missing binary: $BIN" >&2
  exit 1
fi
"$BIN" --bench --orders "${QUENCH_BENCH_ORDERS:-60000}" --seed 42
