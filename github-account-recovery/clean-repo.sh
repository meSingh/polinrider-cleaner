#!/usr/bin/env bash
# clean-repo.sh - remove a committed PolinRider payload from one repository.
# Dry run by default. Thin wrapper; the implementation is ../lib/gh-clean.sh,
# read it before you run it with --apply.
set -uo pipefail
PRC_WRAPPER="$0"; export PRC_WRAPPER
exec "$(cd "$(dirname "$0")" && pwd)/../lib/gh-clean.sh" "$@"
