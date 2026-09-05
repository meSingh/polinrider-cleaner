#!/usr/bin/env bash
# scan.sh - mirror every repository in the organization and scan every ref
# Thin wrapper. The implementation is ../lib/gh-scan.sh - read it, it is the same engine
# used by both the org and the personal workflow.
set -uo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --org) ARGS+=(--owner "$2"); shift 2 ;;
    *)  ARGS+=("$1"); shift ;;
  esac
done
exec "$DIR/../lib/gh-scan.sh" --owner-type org ${ARGS[@]+"${ARGS[@]}"}
