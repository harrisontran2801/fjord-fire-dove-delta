#!/bin/sh
set -eu
BIN="${QUENCH_BINARY:-./app}"
"$BIN" >/dev/null
