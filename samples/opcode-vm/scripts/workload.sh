#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
BIN="${QUENCH_BINARY:-$ROOT/target/release/opcode-vm}"
if [ ! -x "$BIN" ]; then
  echo "profile missing binary: $BIN" >&2
  exit 1
fi
"$BIN" --steps "${QUENCH_PROFILE_STEPS:-80000000}" --seed 7
