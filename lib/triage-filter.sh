#!/usr/bin/env bash
# triage-filter.sh - separate real findings from matches inside detection tooling.
#
# A grep-based scanner cannot tell "this file IS the malware" from "this file
# DETECTS the malware": both contain the same strings. Your own scan workflows,
# this repository, and incident documentation will all be flagged INFECTED. This
# splits them by path so the remaining list is short enough to read by hand.
#
# READ-ONLY. Usage: triage-filter.sh <triage.json> [--strict]
#   --strict   exit 1 if any REAL_SUSPECT remains (for use in CI)

set -uo pipefail
# shellcheck source=lib/common.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

REPORT="${1:-}"; STRICT=0
[[ "$REPORT" == "-h" || "$REPORT" == "--help" ]] && { sed -n '2,12p' "$0"; exit 0; }
[[ "${2:-}" == "--strict" ]] && STRICT=1
[[ -f "$REPORT" ]] || prc_die "usage: triage-filter.sh <triage.json> [--strict]"
prc_need jq

BENIGN_RE="$PRC_BENIGN_RE"   # defined in common.sh, shared with next-steps.sh

jq -c '.[] | select(.verdict=="INFECTED")' "$REPORT" | while read -r entry; do
  repo=$(jq -r .repo <<<"$entry")
  ref=$(jq  -r .ref  <<<"$entry")

  # ioc_strings lines are "ref:path:line:content" - field 2 is the path
  paths=$(jq -r '
    (.ioc_strings[]?     | split(":")[1]),
    (.ioc_filenames[]?   | sub(" .*$";"")),
    (.font_masquerade[]? | sub(" .*$";""))
  ' <<<"$entry" | grep -v '^$' | sort -u)
  [[ -z "$paths" ]] && continue

  risky=""; benign=""
  while read -r p; do
    [[ -z "$p" ]] && continue
    if printf '%s' "$p" | grep -qEi "$BENIGN_RE"; then
      benign="${benign}${p}"$'\n'
    else
      risky="${risky}${p}"$'\n'
    fi
  done <<< "$paths"

  if [[ -z "$risky" ]]; then
    printf 'BENIGN_TOOLING  %s  %s\n' "$repo" "$ref"
    printf '%s' "$benign" | grep -v '^$' | sed 's/^/    (own detection file) /'
  else
    printf 'REAL_SUSPECT    %s  %s\n' "$repo" "$ref"
    printf '%s' "$risky"  | grep -v '^$' | sed 's/^/    (unexplained path)   /'
    [[ -n "$benign" ]] && printf '%s' "$benign" | grep -v '^$' | sed 's/^/    (own detection file) /'
    echo "$repo $ref" >> "${TMPDIR:-/tmp}/.prc-real.$$"
  fi
  echo
done

TOTAL=$(jq '[.[] | select(.verdict=="INFECTED")] | length' "$REPORT")
REALC=$(grep -c . "${TMPDIR:-/tmp}/.prc-real.$$" 2>/dev/null || echo 0)
rm -f "${TMPDIR:-/tmp}/.prc-real.$$"

cat <<EOF
================================================================
 INFECTED verdicts in report : $TOTAL
 REAL_SUSPECT refs           : $REALC
 BENIGN_TOOLING refs         : $(( TOTAL - REALC ))

$(if [[ "${PRC_EMBEDDED:-0}" != "1" ]]; then
  printf '\n Read every REAL_SUSPECT path before concluding anything. Read the\n'
  printf ' matched content, not the count: cat %s/triage.txt\n' "$(dirname "$REPORT")"
fi)
================================================================
EOF

[[ $STRICT -eq 1 && $REALC -gt 0 ]] && exit 1
exit 0
