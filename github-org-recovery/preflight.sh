#!/usr/bin/env bash
# preflight.sh - run immediately before restore.sh --apply. READ-ONLY.
#
# Usage: ./preflight.sh --org ACME --plan evidence/restore-plan.tsv [--actor LOGIN]
#
# Exits 1 if it finds a blocker. Sections 3, 4 and 5 still need your eyes.

set -uo pipefail
# shellcheck source=lib/common.sh
. "$(cd "$(dirname "$0")" && pwd)/../lib/common.sh"

ORG=""; PLAN=""; ACTOR=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --org)   ORG="$2"; shift 2 ;;
    --plan)  PLAN="$2"; shift 2 ;;
    --actor) ACTOR="$2"; shift 2 ;;
    -h|--help) sed -n '2,8p' "$0"; exit 0 ;;
    *) prc_die "unknown argument: $1" ;;
  esac
done
[[ -n "$ORG" && -f "${PLAN:-}" ]] || prc_die "need --org and --plan <restore-plan.tsv>"
prc_need gh

FAIL=0
red() { printf '  [BLOCK] %s\n' "$*"; FAIL=1; }
grn() { printf '  [ok]    %s\n' "$*"; }
yel() { printf '  [check] %s\n' "$*"; }

echo "== 1. Is the attacker's access actually severed? =="
if [[ -n "$ACTOR" ]]; then
  if gh api "/orgs/$ORG/memberships/$ACTOR" >/dev/null 2>&1; then
    red "$ACTOR is still an org member. Restoring now restores into a live session."
    red "  gh api -X DELETE /orgs/$ORG/memberships/$ACTOR"
  else
    grn "$ACTOR is not an active member of $ORG"
  fi
else
  yel "no --actor given, membership not checked"
fi
cat <<EOF
  The API cannot see these. Confirm by hand:
    - their personal access tokens revoked
      https://github.com/organizations/$ORG/settings/personal-access-tokens
    - their OAuth apps and GitHub Apps revoked
    - SSO / email password reset with global sign-out
    - deploy keys on affected repos reviewed: gh api /repos/$ORG/REPO/keys
EOF

echo
echo "== 2. No hostile SHA is a restore target =="
HIT=0
while read -r sha; do
  [[ -z "$sha" ]] && continue
  if awk -F'\t' -v s="$sha" 'NR>1 && $3==s' "$PLAN" | grep -q .; then
    red "restore target equals a SHA pushed during the attack: $sha"; HIT=1
  fi
done < <(awk -F'\t' 'NR>1 {print $4}' "$PLAN" | sort -u)
[[ $HIT -eq 0 ]] && grn "no attacker SHA appears in the restore_to_sha column"

echo
echo "== 3. Payload confirmation, one diff per distinct before -> after pair =="
awk -F'\t' 'NR>1 && $7 ~ /^ok/ {print $1"\t"$3"\t"$4}' "$PLAN" | sort -u | \
while IFS=$'\t' read -r repo before after; do
  printf '  --- %s  %s -> %s\n' "$repo" "${before:0:10}" "${after:0:10}"
  N=$(gh api "/repos/$repo/compare/$before...$after" --jq '.commits | length' 2>/dev/null)
  printf '      commits ahead: %s\n' "${N:-?}"
  F=$(gh api "/repos/$repo/compare/$before...$after" \
        --jq '.files[]? | [.status, (.additions|tostring), (.deletions|tostring), .filename] | @tsv' 2>/dev/null)
  if [[ -z "$F" ]]; then
    echo "      (no file list - divergent histories, expected after a rewrite)"
  else
    printf '%s\n' "$F" | head -25 | sed 's/^/      /'
  fi
done

echo
echo "== 4. What a restore would orphan =="
echo "  ok_fastforward rows drop every commit between before and after:"
awk -F'\t' 'NR>1 && $7=="ok_fastforward" {printf "    %-40s %s\n", $1, $2}' "$PLAN"
yel "confirm with each branch owner that those commits are all attacker commits"

echo
echo "== 5. Can you write? =="
yel "your account must be in the bypass list of any ruleset that restricts updates"
gh api "/orgs/$ORG/rulesets" --jq '.[] | [.id, .name, .enforcement] | @tsv' 2>/dev/null \
  | sed 's/^/    /' || echo "    (org rulesets unavailable on this plan - check per-repo rules)"

echo
if [[ $FAIL -eq 1 ]]; then
  echo "RESULT: BLOCKED. Fix the items above before --apply."
  exit 1
fi
echo "RESULT: no blockers found. Sections 3, 4 and 5 still need your eyes."
