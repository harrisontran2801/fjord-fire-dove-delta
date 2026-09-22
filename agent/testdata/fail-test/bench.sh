#!/bin/sh
set -eu
BIN="${QUENCH_BINARY:-./app}"
"$BIN" | sed -n 's/.*elapsed_ms=/elapsed_ms=/p'
