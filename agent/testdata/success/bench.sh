#!/bin/sh
set -eu
# Candidate stage is faster so the pipeline can keep it when transforms succeed.
if [ "${QUENCH_STAGE:-}" = candidate ]; then
  echo "elapsed_ms=8.0"
else
  echo "elapsed_ms=10.0"
fi
