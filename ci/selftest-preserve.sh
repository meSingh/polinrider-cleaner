#!/usr/bin/env bash
# selftest-preserve.sh - offline tests for fetching pre-attack commits.
#
# The thing being proved: a mirror clone does NOT contain the commit you want to
# restore to. It is unreachable in the origin, so the mirror never fetched it,
# and restore.sh looks for it in the mirror. Everything here runs against a
# local origin with uploadpack.allowAnySHA1InWant set, which is how GitHub
# behaves for objects it has not yet garbage-collected.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/prc-preserve.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT
PASS=0; FAIL=0
ok(){ PASS=$((PASS+1)); printf '  ok   %s\n' "$*"; }
no(){ FAIL=$((FAIL+1)); printf '  FAIL %s\n' "$*"; }

export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@e GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@e

# --- an origin where a branch was force-pushed off a good commit ------------
ORIGIN="$TMP/origin"
git init -q --bare "$ORIGIN"
git -C "$ORIGIN" config uploadpack.allowAnySHA1InWant true   # GitHub does this
WORK="$TMP/work"
git init -q "$WORK" && git -C "$WORK" checkout -q -b master
echo good > "$WORK/f"; git -C "$WORK" add f; git -C "$WORK" commit -qm good
GOOD=$(git -C "$WORK" rev-parse HEAD)
git -C "$WORK" remote add origin "$ORIGIN"; git -C "$WORK" push -q origin master
# Rewrite history the way a backdated amend does, then force it over the top.
echo bad > "$WORK/f"; git -C "$WORK" add f
git -C "$WORK" commit -q --amend -m good --date="2019-07-04T18:02:59"
BAD=$(git -C "$WORK" rev-parse HEAD)
git -C "$WORK" push -q --force origin master
[[ "$GOOD" != "$BAD" ]] && ok "the fixture rewrote history" || no "the fixture rewrote history"

# --- the mirror, as scan.sh makes it ----------------------------------------
EV="$TMP/evidence"; mkdir -p "$EV"
git clone -q --mirror "file://$ORIGIN" "$EV/origin.git"
git -C "$EV/origin.git" cat-file -e "$GOOD" 2>/dev/null \
  && no "a fresh mirror does NOT contain the pre-attack commit" \
  || ok "a fresh mirror does NOT contain the pre-attack commit"
git -C "$EV/origin.git" cat-file -e "$BAD" 2>/dev/null \
  && ok "the mirror does contain the rewritten commit" || no "the mirror does contain the rewritten commit"

printf 'repo\tref\tbefore\tafter\tactor\tcreated_at\tsize\tforced_hint\n' > "$EV/pushes-on-infected-refs.tsv"
printf 'x/origin\trefs/heads/master\t%s\t%s\tmallory\t2026-08-10T21:14:20Z\t0\t\n' "$GOOD" "$BAD" \
  >> "$EV/pushes-on-infected-refs.tsv"
# The header row must not be mistaken for a SHA.
printf 'x/origin\trefs/heads/master\t0000000000000000000000000000000000000000\t%s\tmallory\t2026-08-10T21:15:00Z\t0\t\n' "$BAD" \
  >> "$EV/pushes-on-infected-refs.tsv"

OUT="$("$ROOT/lib/gh-preserve.sh" --out "$EV" 2>&1)"
git -C "$EV/origin.git" cat-file -e "$GOOD" 2>/dev/null \
  && ok "the pre-attack commit is fetched into the mirror" || no "the pre-attack commit is fetched into the mirror"
git -C "$EV/origin.git" rev-parse --verify -q "refs/polinrider/pre-attack/$GOOD" >/dev/null \
  && ok "it is anchored under refs/polinrider/pre-attack" || no "it is anchored under refs/polinrider/pre-attack"

# Anchoring is the point: without a ref, gc throws away what we just fetched.
git -C "$EV/origin.git" gc --prune=now --quiet 2>/dev/null
git -C "$EV/origin.git" cat-file -e "$GOOD" 2>/dev/null \
  && ok "it survives git gc --prune=now" || no "it survives git gc --prune=now"

grep -q 'fetched and anchored' <<<"$OUT" && ok "reports what it fetched" || no "reports what it fetched"
grep -q '0000000000' <<<"$OUT" && no "skips the all-zero before-SHA" || ok "skips the all-zero before-SHA"

# Running it twice must be safe and must say so.
OUT2="$("$ROOT/lib/gh-preserve.sh" --out "$EV" 2>&1)"
grep -q 'already in mirror    : 1' <<<"$OUT2" && ok "second run is idempotent" || no "second run is idempotent"

# --- a commit GitHub has already collected ----------------------------------
EV2="$TMP/ev2"; mkdir -p "$EV2"
git clone -q --mirror "file://$ORIGIN" "$EV2/origin.git"
printf 'repo\tref\tbefore\tafter\tactor\tcreated_at\tsize\tforced_hint\n' > "$EV2/pushes-on-infected-refs.tsv"
printf 'x/origin\trefs/heads/master\t%s\t%s\tmallory\t2026-08-10T21:14:20Z\t0\t\n' \
  "dead0000dead0000dead0000dead0000dead0000" "$BAD" >> "$EV2/pushes-on-infected-refs.tsv"
OUT3="$("$ROOT/lib/gh-preserve.sh" --out "$EV2" 2>&1)"
grep -q 'GONE from GitHub' <<<"$OUT3" && ok "an unavailable commit is reported GONE" || no "an unavailable commit is reported GONE"
grep -q 'cannot be restored' <<<"$OUT3" && ok "says what GONE means for the fix" || no "says what GONE means for the fix"

# --- a repository with no mirror --------------------------------------------
EV3="$TMP/ev3"; mkdir -p "$EV3"
printf 'repo\tref\tbefore\tafter\tactor\tcreated_at\tsize\tforced_hint\n' > "$EV3/pushes-on-infected-refs.tsv"
printf 'x/absent\trefs/heads/master\t%s\t%s\tmallory\t2026-08-10T21:14:20Z\t0\t\n' "$GOOD" "$BAD" \
  >> "$EV3/pushes-on-infected-refs.tsv"
OUT4="$("$ROOT/lib/gh-preserve.sh" --out "$EV3" 2>&1)"
grep -q 'no mirror' <<<"$OUT4" \
  && ok "a missing mirror is reported, not crashed on" || no "a missing mirror is reported, not crashed on"
grep -qE '^[[:space:]]*repo[[:space:]]' <<<"$OUT4" \
  && no "a header row is not treated as a commit" || ok "a header row is not treated as a commit"

"$ROOT/lib/gh-preserve.sh" --out "$TMP/nothing-here" >/dev/null 2>&1 \
  && no "a missing evidence directory is an error" || ok "a missing evidence directory is an error"
grep -qE '^[^#]*git .*[[:space:]]push[[:space:]]' "$ROOT/lib/gh-preserve.sh" \
  && no "gh-preserve never pushes" || ok "gh-preserve never pushes"

printf '\n  passed %s, failed %s\n' "$PASS" "$FAIL"
[[ "$FAIL" -eq 0 ]] || exit 1
