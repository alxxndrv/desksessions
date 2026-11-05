#!/usr/bin/env bash
# Runs every firmware gate: formatting, lints, tests (golden scenarios included) and, when the
# `esp` toolchain is installed, the device builds. Exits non-zero on any failure.
set -euo pipefail
cd "$(dirname "$0")/.."
exec cargo xtask check
