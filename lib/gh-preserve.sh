#!/usr/bin/env bash
# gh-preserve.sh - pull the pre-attack commits into the local mirrors.
#
# A mirror clone fetches only what is reachable from a ref. After a force-push
# the commit you want to restore to is reachable from nothing, so it is NOT in
# your mirror, even though the mirror is where restore.sh looks for it. GitHub
# still serves those objects by SHA for a while, until it garbage-collects them.
#
# This fetches each pre-attack commit by SHA and anchors it under
# refs/polinrider/pre-attack/, which does two things: restore.sh can find it,
# and it stops being unreachable locally so nothing prunes it.
#
# Do this while the objects are still on GitHub. Nothing brings them back after
# that.
#
# It also checks each commit it recovers for the payload, because a restore
# target can itself be infected. This campaign arrives in waves: the commit
# immediately before the most recent hostile push is often just the previous
# wave. Restoring to it puts the payload back. The verdicts go to
# restore-targets.tsv, earliest first, and the earliest CLEAN one is the commit
# to restore to.
#
# READ-ONLY with respect to GitHub: it fetches, it never pushes.
#
# Usage:
#   gh-preserve.sh [--out DIR]

set -uo pipefail
# shellcheck source=lib/common.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

OUT=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --out)     OUT="$2"; shift 2 ;;
    -h|--help) sed -n '2,19p' "$0"; exit 0 ;;
    *) prc_die "unknown argument: $1" ;;
  esac
done
OUT="${OUT:-$(prc_default_evidence_dir)}"
[[ -d "$OUT" ]] || prc_die "no evidence directory at $OUT - run the scan first"
prc_need git

HOSTILE="$OUT/pushes-on-infected-refs.tsv"
[[ -f "$HOSTILE" ]] || prc_die "missing $HOSTILE - run the scan, it writes this"

printf '\nPreserving pre-attack commits\n'
printf '  from %s\n\n' "$HOSTILE"

TARGETS="$OUT/restore-targets.tsv"
: > "$TARGETS"
KEPT=0; ALREADY=0; GONE=0; NOMIRROR=0; CLEAN=0; DIRTY=0

# prc_commit_verdict <mirror> <sha> - CLEAN or INFECTED. Strong indicators only,
# plus a .woff2 that does not begin with the wOF2 magic, which is the shape the
# campaign uses to smuggle a script past a binary-looking filename.
commit_verdict() {
  local m="$1" sha="$2" f magic
  if git -C "$m" grep -l -F \
       -e 'rmcej%otb%' -e 'Cot%3t=shtP' -e '_$_1e42' \
       -e 'global["_V"]' -e "global['_V']" "$sha" >/dev/null 2>&1; then
    printf 'INFECTED\n'; return 0
  fi
  while IFS= read -r f; do
    [[ -z "$f" ]] && continue
    magic=$(git -C "$m" cat-file blob "$sha:$f" 2>/dev/null | head -c 4 | od -An -tx1 | tr -d ' \n')
    [[ "$magic" == "774f4632" ]] || { printf 'INFECTED\n'; return 0; }
  done < <(git -C "$m" ls-tree -r --name-only "$sha" 2>/dev/null | grep '\.woff2$')
  printf 'CLEAN\n'
}
# repo <tab> ref <tab> before <tab> after ... ; the "before" of each push is a
# candidate restore point. Deduplicate: one repo is pushed to many times.
while IFS=$'\t' read -r repo _ref before _after _actor when _rest; do
  # Skip anything that is not a real object id: a header row if the file was
  # written with one, the all-zero "before" of a branch creation, and blanks.
  case "$before" in
    [0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]*) ;;
    *) continue ;;
  esac
  [[ "${#before}" -eq 40 || "${#before}" -eq 64 ]] || continue
  [[ "$before" =~ ^0+$ ]] && continue
  [[ "$repo" == */* ]] || continue
  mirror="$OUT/${repo##*/}.git"
  if [[ ! -d "$mirror" ]]; then
    printf '  %-34s %-12s no mirror\n' "$repo" "${before:0:10}"
    NOMIRROR=$((NOMIRROR+1)); continue
  fi
  if git -C "$mirror" cat-file -e "$before" 2>/dev/null; then
    git -C "$mirror" update-ref "refs/polinrider/pre-attack/$before" "$before" 2>/dev/null || true
    v=$(commit_verdict "$mirror" "$before")
    printf '%s\t%s\t%s\t%s\n' "$repo" "$before" "$v" "$when" >> "$TARGETS"
    printf '  %-34s %-12s already here   %s\n' "$repo" "${before:0:10}" "$v"
    [[ "$v" == CLEAN ]] && CLEAN=$((CLEAN+1)) || DIRTY=$((DIRTY+1))
    ALREADY=$((ALREADY+1)); continue
  fi
  if git -C "$mirror" fetch --quiet origin "$before" 2>/dev/null \
     && git -C "$mirror" cat-file -e "$before" 2>/dev/null; then
    # Anchor it. Without a ref it is unreachable locally too, and git will
    # eventually prune the very thing we just went to get.
    if git -C "$mirror" update-ref "refs/polinrider/pre-attack/$before" "$before"; then
      v=$(commit_verdict "$mirror" "$before")
      printf '%s\t%s\t%s\t%s\n' "$repo" "$before" "$v" "$when" >> "$TARGETS"
      printf '  %-34s %-12s recovered      %s\n' "$repo" "${before:0:10}" "$v"
      [[ "$v" == CLEAN ]] && CLEAN=$((CLEAN+1)) || DIRTY=$((DIRTY+1))
      KEPT=$((KEPT+1))
    fi
  else
    printf '%s\t%s\tGONE\t%s\n' "$repo" "$before" "$when" >> "$TARGETS"
    printf '  %-34s %-12s GONE from GitHub\n' "$repo" "${before:0:10}"
    GONE=$((GONE+1))
  fi
done < <(awk -F'\t' '!seen[$1"|"$3]++' "$HOSTILE")

# The recommendation: earliest CLEAN commit per repository. "Earliest" matters.
# The commit immediately before the last hostile push is often just the previous
# wave, and restoring to it puts the payload straight back.
if [[ -s "$TARGETS" ]]; then
  sort -t"$(printf '\t')" -k1,1 -k4,4 -o "$TARGETS" "$TARGETS"
  printf '\nRestore targets\n'
  awk -F'\t' '$3=="CLEAN" && !seen[$1]++ {printf "  %-34s %s  restore to this one\n", $1, substr($2,1,10)}' "$TARGETS"
  awk -F'\t' '$3!="CLEAN"{bad[$1]++} $3=="CLEAN"{good[$1]++}
    END{for (r in bad) if (!(r in good)) printf "  %-34s no clean target, remove the payload instead\n", r}' "$TARGETS"
fi

printf '\n  fetched and anchored : %s\n' "$KEPT"
printf '  already in mirror    : %s\n'   "$ALREADY"
printf '  no longer on GitHub  : %s\n'   "$GONE"
printf '  clean targets        : %s\n'   "$CLEAN"
printf '  targets already bad  : %s\n'   "$DIRTY"
[[ "$NOMIRROR" -gt 0 ]] && printf '  no local mirror      : %s\n' "$NOMIRROR"

if [[ "$GONE" -gt 0 ]]; then
  printf '\n  %s commit(s) are gone. GitHub has garbage-collected them and there is\n' "$GONE"
  printf '  no way to get them back. Those branches cannot be restored; remove the\n'
  printf '  payload with clean-repo.sh instead.\n'
fi
if [[ "$DIRTY" -gt 0 ]]; then
  printf '\n  %s recovered commit(s) already carry the payload. That is what a second\n' "$DIRTY"
  printf '  wave looks like: the commit before the last hostile push is the previous\n'
  printf '  wave, not a clean state. Restore to the earliest CLEAN commit above, and\n'
  printf '  never to one listed as INFECTED.\n'
fi
if [[ "$KEPT" -gt 0 || "$ALREADY" -gt 0 ]]; then
  printf '\n  %s restore point(s) are now safe in your mirrors and will survive a\n' "$((KEPT + ALREADY))"
  printf '  re-scan. They will not survive the evidence directory being cleared.\n'
fi
exit 0
