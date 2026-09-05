#!/usr/bin/env bash
# selftest-restore.sh - offline test of the restore planner.
#
# Builds a repository with a known-good commit and two attacker commits, plus a
# push ledger describing a two-wave attack, then asserts that the planner
# classifies every row correctly and never targets a commit from the attack
# window. No network and no credentials: every row resolves from the local
# mirror, so no GitHub API call is made.
#
# Usage: ./selftest-restore.sh

set -uo pipefail
SDIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SDIR/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
FAILED=0

pass() { printf '  ok    %s\n' "$*"; }
fail() { printf '  FAIL  %s\n' "$*"; FAILED=1; }

# --- fixture ---------------------------------------------------------------
mkdir -p "$TMP/src" "$TMP/evidence"
(
  cd "$TMP/src" || exit 1
  git init -q .
  git config user.email selftest@example.invalid
  git config user.name selftest
  echo good > a.txt; git add .; git commit -qm "last known good"
  echo wave1 >> a.txt; git commit -qam "attacker wave 1"
  echo wave2 >> a.txt; git commit -qam "attacker wave 2"
) || { echo "fixture setup failed"; exit 1; }

GOOD=$(git -C "$TMP/src" rev-parse HEAD~2)
W1=$(git   -C "$TMP/src" rev-parse HEAD~1)
W2=$(git   -C "$TMP/src" rev-parse HEAD)
git clone -q --mirror "$TMP/src" "$TMP/evidence/repoa.git"

LEDGER="$TMP/evidence/pushes.tsv"
printf 'repo\tref\tbefore\tafter\tactor\tcreated_at\tsize\thint\n' > "$LEDGER"
{
  # in-window, restorable: before is the last good commit
  printf 'acme/repoa\trefs/heads/main\t%s\t%s\tbadguy\t2026-07-27T04:36:51Z\t0\t\n' "$GOOD" "$W1"
  # in-window, second wave: its "before" IS the first wave's malicious commit
  printf 'acme/repoa\trefs/heads/dev\t%s\t%s\tbadguy\t2026-07-27T05:35:34Z\t0\t\n' "$W1" "$W2"
  # in-window but no local mirror for that repository
  printf 'acme/repob\trefs/heads/main\t%s\t%s\tbadguy\t2026-07-27T04:40:00Z\t0\t\n' "$GOOD" "$W1"
  # before the window: a legitimate push that must be ignored entirely
  printf 'acme/repoa\trefs/heads/old\t%s\t%s\tdeveloper\t2026-07-20T09:00:00Z\t3\t\n' "$GOOD" "$W1"
} >> "$LEDGER"

# --- run -------------------------------------------------------------------
OUT="$("$ROOT/lib/gh-restore.sh" --ledger "$LEDGER" --mirrors "$TMP/evidence" \
        --since 2026-07-27T03:00:00Z 2>&1)"
RC=$?
PLAN="$TMP/evidence/restore-plan.tsv"
printf '%s\n' "$OUT" | sed 's/^/  | /'
echo
echo "assertions:"

if [[ $RC -eq 0 ]]; then pass "dry run exits 0"; else fail "expected exit 0, got $RC"; fi

status_of() { awk -F'\t' -v b="$1" 'NR>1 && $2==b {print $7}' "$PLAN" | head -1; }
target_of() { awk -F'\t' -v b="$1" 'NR>1 && $2==b {print $3}' "$PLAN" | head -1; }

if [[ "$(status_of main)" == "ok_fastforward" ]]; then
  pass "main classified ok_fastforward"
else fail "main: expected ok_fastforward, got '$(status_of main)'"; fi

if [[ "$(status_of dev)" == "MALICIOUS_TARGET" ]]; then
  pass "second wave refused as MALICIOUS_TARGET"
else fail "dev: expected MALICIOUS_TARGET, got '$(status_of dev)'"; fi

if [[ "$(target_of main)" == "$GOOD" ]]; then
  pass "main restores to the last known good commit"
else fail "main target is '$(target_of main)', expected $GOOD"; fi

if awk -F'\t' 'NR>1 && $7 ~ /^ok/ {print $3}' "$PLAN" | grep -qxF "$W1"; then
  fail "a wave-1 malicious commit is an approved restore target"
else pass "no malicious commit appears as an approved restore target"; fi

if [[ -z "$(status_of old)" ]]; then
  pass "pre-window push excluded from the plan"
else fail "pre-window push leaked into the plan as '$(status_of old)'"; fi

if [[ "$(awk -F'\t' 'NR>1 && $1=="acme/repob" {print $7}' "$PLAN" | head -1)" == "NO_MIRROR" ]]; then
  pass "missing mirror reported as NO_MIRROR"
else fail "repob: expected NO_MIRROR"; fi

if printf '%s' "$OUT" | grep -q "DRY RUN. Nothing changed."; then
  pass "dry run states it changed nothing"
else fail "dry run did not print the no-change notice"; fi

if [[ "$(git -C "$TMP/evidence/repoa.git" rev-parse refs/heads/master 2>/dev/null)" == "$W2" ]]; then
  pass "the mirror was not modified"
else fail "the mirror was modified by a dry run"; fi

echo
if [[ $FAILED -eq 0 ]]; then echo "RESTORE SELFTEST PASSED"; exit 0; fi
echo "RESTORE SELFTEST FAILED"; exit 1
