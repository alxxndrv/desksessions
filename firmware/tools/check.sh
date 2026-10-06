#!/usr/bin/env bash
# Runs every firmware gate: unit tests and the simulator build. Exits non-zero on any failure.
set -euo pipefail
cd "$(dirname "$0")/.."

pio test -e native_test | tail -n 1
pio run -e sim > /tmp/desk-sim-build.log 2>&1 || { grep -E "error" /tmp/desk-sim-build.log; exit 1; }
echo "sim build ok"
