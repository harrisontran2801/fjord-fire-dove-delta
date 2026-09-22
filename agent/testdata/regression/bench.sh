#!/bin/sh
set -eu
# Candidate stage is slower on purpose so the pipeline must discard it.
if [ "${QUENCH_STAGE:-}" = candidate ]; then
  echo "elapsed_ms=50.0"
else
  echo "elapsed_ms=10.0"
fi
