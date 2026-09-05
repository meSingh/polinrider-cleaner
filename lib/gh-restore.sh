#!/usr/bin/env bash
# gh-restore.sh - restore every ref to the commit that existed immediately before
#                 the first hostile push. History is not rewritten and no work is
#                 deleted: the branch pointer is moved back, and the malicious
#                 commits become unreachable.
#
# DRY RUN BY DEFAULT. It prints a plan and changes nothing. --apply executes it.
#
# Usage:
#   gh-restore.sh --sweep evidence/sweep.tsv --mirrors evidence --since <T0> [options]
#   gh-restore.sh --ledger evidence/pushes.tsv --mirrors evidence --since <T0> [options]
#
# Options:
#   --actor LOGIN    only treat pushes by this login as hostile
#   --repo OWNER/N   restrict to one repository
#   --apply          perform the restore
#
# Statuses in the plan:
#   ok                 history was rewritten by that push. Definite force push.
#   ok_fastforward     commits were appended, no rewrite. Restorable, read the diff
#                      first: a legitimate push in the window looks the same.
#   ok_orphaned        the target commit is unreachable from any current ref, so
#                      the mirror never fetched it, but GitHub still holds it and
#                      confirmed so. Fully restorable. This is the NORMAL state for
#                      a branch the attacker moved. It does not mean work was lost.
#   MALICIOUS_TARGET   the target is itself a SHA pushed inside the window. This is
#                      the second-wave trap. Never restored. Re-run with a --since
#                      that starts before the first wave.
#   SHA_GONE           the commit returned 404. It has been garbage collected.
#                      That branch needs delete-and-recreate instead.
#   NO_MIRROR          no local mirror. Re-run the scan for that repository.

set -uo pipefail
# shellcheck source=lib/common.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

LEDGER=""; SWEEP=""; MIRRORS=""; SINCE=""; ACTOR=""; ONLY_REPO=""; APPLY=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --sweep)   SWEEP="$2"; shift 2 ;;
    --ledger)  LEDGER="$2"; shift 2 ;;
    --mirrors) MIRRORS="$2"; shift 2 ;;
    --since)   SINCE="$2"; shift 2 ;;
    --actor)   ACTOR="$2"; shift 2 ;;
    --repo)    ONLY_REPO="$2"; shift 2 ;;
    --apply)   APPLY=1; shift ;;
    -h|--help) sed -n '2,32p' "$0"; exit 0 ;;
    *) prc_die "unknown argument: $1" ;;
  esac
done

# sweep.tsv is  time repo event actor ref before after size
# pushes.tsv is repo ref before after actor created_at size hint
if [[ -n "$SWEEP" ]]; then
  [[ -f "$SWEEP" ]] || prc_die "no such file: $SWEEP"
  LEDGER="$(dirname "$SWEEP")/pushes.tsv"
  awk -F'\t' 'NR==1 { print "repo\tref\tbefore\tafter\tactor\tcreated_at\tsize\thint"; next }
              $3=="PushEvent" && $6 != "" && $6 !~ /^0+$/ {
                print $2"\t"$5"\t"$6"\t"$7"\t"$4"\t"$1"\t"$8"\t" }' "$SWEEP" > "$LEDGER"
  printf 'normalised %s push events -> %s\n' "$(( $(grep -c . "$LEDGER") - 1 ))" "$LEDGER" >&2
fi

[[ -f "$LEDGER"  ]] || prc_die "need --sweep <sweep.tsv> or --ledger <pushes.tsv>"
[[ -d "$MIRRORS" ]] || prc_die "need --mirrors <directory of *.git>"
[[ -n "$SINCE"   ]] || prc_die "need --since <ISO8601 T0>"
prc_need git gh

PLAN="$(dirname "$LEDGER")/restore-plan.tsv"
CACHE="$(dirname "$LEDGER")/.sha-liveness.cache"
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
: > "$CACHE"; : > "$WORK/seen"

# Every SHA pushed inside the window is hostile. A restore must never target one.
awk -F'\t' -v since="$SINCE" -v actor="$ACTOR" -v only="$ONLY_REPO" '
  NR>1 && $6 >= since && (actor=="" || $5==actor) && (only=="" || $1==only) {print $4}' \
  "$LEDGER" | sort -u | grep -v '^$' > "$WORK/hostile"
printf 'hostile SHAs seen in window: %s\n' "$(grep -c . "$WORK/hostile")" >&2

printf 'repo\tbranch\trestore_to_sha\tattacker_sha\tactor\tpush_time\tstatus\n' > "$PLAN"

# Oldest first, so the FIRST hostile push per ref wins. Its `before` is last-known-good.
# shellcheck disable=SC2034  # size and hint are ledger columns this loop ignores
tail -n +2 "$LEDGER" | sort -t"$(printf '\t')" -k6,6 | \
while IFS=$'\t' read -r repo ref before after actor created size hint; do
  [[ -z "$repo" || -z "$ref" ]] && continue
  [[ -n "$ONLY_REPO" && "$repo"  != "$ONLY_REPO" ]] && continue
  [[ -n "$ACTOR"     && "$actor" != "$ACTOR"     ]] && continue
  [[ "$created" < "$SINCE" ]] && continue
  [[ -z "$before" || "$before" =~ ^0+$ ]] && continue

  branch="${ref#refs/heads/}"
  grep -qxF "${repo}|${branch}" "$WORK/seen" && continue
  echo "${repo}|${branch}" >> "$WORK/seen"

  mirror="$MIRRORS/${repo##*/}.git"
  status="ok"

  if grep -qxF "$before" "$WORK/hostile"; then
    status="MALICIOUS_TARGET"
  elif [[ ! -d "$mirror" ]]; then
    status="NO_MIRROR"
  elif git -C "$mirror" cat-file -e "$before" 2>/dev/null; then
    git -C "$mirror" merge-base --is-ancestor "$before" "$after" 2>/dev/null \
      && status="ok_fastforward"
  else
    # Not in the mirror is expected: `git clone --mirror` only fetches objects
    # reachable from a ref, and the attacker moved every ref off this commit.
    # GitHub still holds unreachable objects until GC, so ask the API.
    if   grep -qxF "${repo}|${before}|alive" "$CACHE"; then status="ok_orphaned"
    elif grep -qxF "${repo}|${before}|gone"  "$CACHE"; then status="SHA_GONE"
    elif gh api "/repos/${repo}/commits/${before}" --jq .sha >/dev/null 2>&1; then
      echo "${repo}|${before}|alive" >> "$CACHE"; status="ok_orphaned"
    else
      echo "${repo}|${before}|gone"  >> "$CACHE"; status="SHA_GONE"
    fi
  fi

  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$repo" "$branch" "$before" "$after" "$actor" "$created" "$status" >> "$PLAN"
done

echo
echo "Restore plan: $PLAN"
column -t -s"$(printf '\t')" "$PLAN" 2>/dev/null || cat "$PLAN"
echo

OKC=$(awk -F'\t' 'NR>1 && $7 ~ /^ok/'  "$PLAN" | grep -c . )
BADC=$(awk -F'\t' 'NR>1 && $7 !~ /^ok/' "$PLAN" | grep -c . )
printf 'restorable: %s    needs another path: %s\n' "$OKC" "$BADC"
if [[ "$BADC" -gt 0 ]]; then
  echo
  echo "Not restorable by this plan:"
  awk -F'\t' 'NR>1 && $7 !~ /^ok/ {printf "  %-40s %-30s %s\n", $1, $2, $7}' "$PLAN"
fi
echo

if [[ $APPLY -eq 0 ]]; then
  cat <<'EOF'
DRY RUN. Nothing changed.

Before re-running with --apply:
  1. The account that made the hostile pushes must be locked out already. If its
     credentials are still live, a restore is re-pushed within minutes.
  2. Your own account must be in the bypass list of any ruleset that restricts
     updates, or every restore returns 422.
  3. Every row must be a push nobody on the team claims. Ask, do not assume.
  4. Rows marked ok_fastforward drop every commit between before and after.
     Confirm with the branch owner that all of them are attacker commits.
  5. Legitimate work pushed after the hostile push on a branch becomes orphaned.
     It still exists in the mirror. Cherry-pick it forward afterwards.

Then: same command again with --apply
EOF
  exit 0
fi

echo "APPLYING in 5 seconds. Ctrl-C to abort."
sleep 5

FAILED=0
while IFS=$'\t' read -r repo branch sha; do
  [[ -z "$repo" ]] && continue
  printf 'restoring %-40s %-30s -> %s ... ' "$repo" "$branch" "${sha:0:10}"
  if gh api -X PATCH "/repos/${repo}/git/refs/heads/${branch}" \
       -f sha="$sha" -F force=true >/dev/null 2>&1; then
    echo "done"
  else
    echo "FAILED (check ruleset bypass and token scope)"
    FAILED=$((FAILED+1))
  fi
done < <(awk -F'\t' 'NR>1 && $7 ~ /^ok/ {print $1"\t"$2"\t"$3}' "$PLAN")

echo
[[ $FAILED -gt 0 ]] && echo "$FAILED restore(s) failed. Re-run after fixing the cause."
echo "Now re-scan before trusting anything. Expect INFECTED 0."
