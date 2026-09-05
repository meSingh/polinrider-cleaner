#!/usr/bin/env bash
# sweep.sh - list every ref-touching event in the organization since T0
# Thin wrapper. The implementation is ../lib/gh-sweep.sh - read it, it is the same engine
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
exec "$DIR/../lib/gh-sweep.sh" --owner-type org ${ARGS[@]+"${ARGS[@]}"}
