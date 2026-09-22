#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
BIN="${QUENCH_BINARY:-$ROOT/target/release/match-engine}"
if [ ! -x "$BIN" ]; then
  echo "profile missing binary: $BIN" >&2
  exit 1
fi
"$BIN" --orders "${QUENCH_PROFILE_ORDERS:-120000}" --seed 7
