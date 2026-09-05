#!/usr/bin/env bash
# preflight.sh - run immediately before restore.sh --apply. READ-ONLY.
#
# Usage: ./preflight.sh --plan evidence/restore-plan.tsv
#
# On a personal account the hostile pushes carry YOUR login, because they were
# made with your stolen credentials. Actor filtering proves nothing here, so this
# checks the credential surface instead. Exits 1 if it finds a blocker.

set -uo pipefail
# shellcheck source=lib/common.sh
. "$(cd "$(dirname "$0")" && pwd)/../lib/common.sh"

PLAN=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --plan) PLAN="$2"; shift 2 ;;
    -h|--help) sed -n '2,9p' "$0"; exit 0 ;;
    *) prc_die "unknown argument: $1" ;;
  esac
done
[[ -f "${PLAN:-}" ]] || prc_die "need --plan <restore-plan.tsv>"
prc_need gh

FAIL=0
red() { printf '  [BLOCK] %s\n' "$*"; FAIL=1; }
grn() { printf '  [ok]    %s\n' "$*"; }
yel() { printf '  [check] %s\n' "$*"; }

ME="$(gh api /user --jq .login 2>/dev/null || true)"
[[ -n "$ME" ]] || prc_die "gh is not authenticated. Run: gh auth login"
echo "authenticated as: $ME"

echo
echo "== 1. Credentials rotated before restoring? =="
echo "  SSH and signing keys currently on the account:"
gh api /user/keys     --jq '.[] | ["auth   ", (.id|tostring), .title] | @tsv' 2>/dev/null | sed 's/^/    /'
gh api /user/gpg_keys --jq '.[] | ["signing", (.key_id // ""), (.name // "")] | @tsv' 2>/dev/null | sed 's/^/    /'
cat <<'EOF'
  Delete any key you do not recognise, then confirm by hand:
    - every personal access token revoked   https://github.com/settings/tokens
    - authorised OAuth apps reviewed        https://github.com/settings/applications
    - password changed with "sign out of all other sessions"
    - the machine that was infected is rebuilt, not cleaned
  If the credential that made these pushes is still valid, the restore is
  re-pushed within minutes.
EOF
yel "confirm the four items above yourself, the API cannot"

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
yel "your own work from that window can be in there. Read section 3 before applying."

echo
echo "== 5. Branch protection that would reject the restore =="
awk -F'\t' 'NR>1 && $7 ~ /^ok/ {print $1}' "$PLAN" | sort -u | while read -r repo; do
  R=$(gh api "/repos/$repo/rulesets" --jq '.[] | [.name, .enforcement] | @tsv' 2>/dev/null)
  [[ -n "$R" ]] && printf '    %-40s %s\n' "$repo" "$(printf '%s' "$R" | tr '\n' ' ')"
done
yel "if a restore returns 422, that is a ruleset. Disable it, restore, re-enable it."

echo
if [[ $FAIL -eq 1 ]]; then
  echo "RESULT: BLOCKED. Fix the items above before --apply."
  exit 1
fi
echo "RESULT: no blockers found. Sections 1, 3 and 4 still need your eyes."
