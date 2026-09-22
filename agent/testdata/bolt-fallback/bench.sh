#!/bin/sh
set -eu
# Same elapsed for baseline and candidate so a no-op BOLT rewrite is rejected
# by min_improvement_percent. Do not invent a keep.
echo "elapsed_ms=10.0"
