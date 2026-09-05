#!/usr/bin/env bash
# triage-filter.sh - drop findings that are your own detection tooling matching itself.
set -uo pipefail
exec "$(cd "$(dirname "$0")" && pwd)/../lib/triage-filter.sh" "$@"
