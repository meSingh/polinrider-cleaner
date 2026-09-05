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

KEPT=0; ALREADY=0; GONE=0; NOMIRROR=0
# repo <tab> ref <tab> before <tab> after ... ; the "before" of each push is a
# candidate restore point. Deduplicate: one repo is pushed to many times.
while IFS=$'\t' read -r repo _ref before _rest; do
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
    printf '  %-34s %-12s already here\n' "$repo" "${before:0:10}"
    ALREADY=$((ALREADY+1)); continue
  fi
  if git -C "$mirror" fetch --quiet origin "$before" 2>/dev/null \
     && git -C "$mirror" cat-file -e "$before" 2>/dev/null; then
    # Anchor it. Without a ref it is unreachable locally too, and git will
    # eventually prune the very thing we just went to get.
    git -C "$mirror" update-ref "refs/polinrider/pre-attack/$before" "$before" \
      && printf '  %-34s %-12s fetched and anchored\n' "$repo" "${before:0:10}" \
      && KEPT=$((KEPT+1))
  else
    printf '  %-34s %-12s GONE from GitHub\n' "$repo" "${before:0:10}"
    GONE=$((GONE+1))
  fi
done < <(awk -F'\t' '!seen[$1"|"$3]++' "$HOSTILE")

printf '\n  fetched and anchored : %s\n' "$KEPT"
printf '  already in mirror    : %s\n'   "$ALREADY"
printf '  no longer on GitHub  : %s\n'   "$GONE"
[[ "$NOMIRROR" -gt 0 ]] && printf '  no local mirror      : %s\n' "$NOMIRROR"

if [[ "$GONE" -gt 0 ]]; then
  printf '\n  %s commit(s) are gone. GitHub has garbage-collected them and there is\n' "$GONE"
  printf '  no way to get them back. Those branches cannot be restored; remove the\n'
  printf '  payload with clean-repo.sh instead.\n'
fi
if [[ "$KEPT" -gt 0 || "$ALREADY" -gt 0 ]]; then
  printf '\n  %s restore point(s) are now safe in your mirrors and will survive a\n' "$((KEPT + ALREADY))"
  printf '  re-scan. They will not survive the evidence directory being cleared.\n'
fi
exit 0
