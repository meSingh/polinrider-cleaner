#!/usr/bin/env bash
# restore.sh - move each branch back to its pre-attack commit. Dry run by default.
# Thin wrapper. The implementation is ../lib/gh-restore.sh - read it before you
# run it with --apply.
set -uo pipefail
exec "$(cd "$(dirname "$0")" && pwd)/../lib/gh-restore.sh" "$@"
