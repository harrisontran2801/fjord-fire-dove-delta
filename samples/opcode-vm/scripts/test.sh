#!/bin/sh
set -eu
ROOT="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$ROOT"
cargo test
BIN="${QUENCH_BINARY:-$ROOT/target/release/opcode-vm}"
if [ ! -x "$BIN" ]; then
  echo "verification missing binary: $BIN" >&2
  exit 1
fi
"$BIN" --self-test
