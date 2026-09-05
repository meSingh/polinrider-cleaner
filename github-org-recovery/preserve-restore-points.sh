#!/usr/bin/env bash
# preserve-restore-points.sh - fetch the pre-attack commits into your mirrors
# before GitHub garbage-collects them. Read-only against GitHub.
# Thin wrapper. The implementation is ../lib/gh-preserve.sh.
set -uo pipefail
PRC_WRAPPER="$0"; export PRC_WRAPPER
exec "$(cd "$(dirname "$0")" && pwd)/../lib/gh-preserve.sh" "$@"
