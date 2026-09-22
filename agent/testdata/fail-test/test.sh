#!/bin/sh
set -eu
BIN="${QUENCH_BINARY:-./app}"
# Baseline (unstripped) must keep the named symbol. After strip the symbol is gone
# and this test fails — the pipeline must discard the candidate.
if ! command -v readelf >/dev/null 2>&1; then
  echo "readelf missing" >&2
  exit 1
fi
readelf -s "$BIN" | grep -q quench_keep_me
