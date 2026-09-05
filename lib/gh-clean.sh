#!/usr/bin/env bash
# gh-clean.sh - remove a committed payload from every affected branch of one repo.
#
# For the case where the payload was committed normally rather than force-pushed,
# so there is no earlier state to restore to. This adds a commit that deletes the
# flagged paths. It never rewrites history and never force-pushes, so the change
# is an ordinary commit you can revert.
#
# It works in a BARE clone using git plumbing. Nothing is ever checked out, so
# the payload never exists as a live file on your disk and no editor or task
# runner can reach it.
#
# DRY RUN BY DEFAULT. Pass --apply to push.
#
# Usage:
#   gh-clean.sh OWNER/REPO [--apply] [--out DIR]
#
# Reads affected-paths.tsv and affected-refs.tsv from the evidence directory,
# both written by next-steps.sh.

set -uo pipefail
# shellcheck source=lib/common.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

NWO=""; APPLY=0; OUT=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply)   APPLY=1; shift ;;
    --out)     OUT="$2"; shift 2 ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    -*) prc_die "unknown argument: $1" ;;
    *)  NWO="$1"; shift ;;
  esac
done
[[ -n "$NWO" ]] || prc_die "usage: gh-clean.sh OWNER/REPO [--apply] [--out DIR]"
[[ "$NWO" == */* ]] || prc_die "expected OWNER/REPO, got: $NWO"
OUT="${OUT:-$(prc_default_evidence_dir)}"
prc_need git gh

# The bare clone of an infected repository must not land inside a checkout.
prc_assert_safe_out "$OUT"

PATHS="$OUT/affected-paths.tsv"
REFS="$OUT/affected-refs.tsv"
for f in "$PATHS" "$REFS"; do
  [[ -f "$f" ]] || prc_die "missing $f - run the scan first, it writes this"
done

mapfile_paths=$(awk -F'\t' -v r="$NWO" '$1==r {print $2}' "$PATHS" | sort -u)
mapfile_refs=$(awk  -F'\t' -v r="$NWO" '$1==r {print $2}' "$REFS"  | sort -u)
[[ -n "$mapfile_paths" ]] || prc_die "no flagged paths recorded for $NWO"
[[ -n "$mapfile_refs"  ]] || prc_die "no flagged branches recorded for $NWO"

NPATHS=$(printf '%s\n' "$mapfile_paths" | grep -c .)
NREFS=$(printf  '%s\n' "$mapfile_refs"  | grep -c .)

printf '\n%s\n' "$NWO"
printf '  %s path(s) to remove, across %s branch(es)\n' "$NPATHS" "$NREFS"
printf '%s\n' "$mapfile_paths" | sed 's/^/    /'
if [[ $APPLY -eq 0 ]]; then
  printf '\n  DRY RUN. Nothing is pushed. Add --apply to do it.\n'
else
  printf '\n  APPLY. Each branch below gets one commit and a normal push.\n'
fi

# --- the bare clone ---------------------------------------------------------
WORK="$OUT/clean/$(printf '%s' "$NWO" | tr '/' '_').git"
mkdir -p "$(dirname "$WORK")" || prc_die "cannot create $(dirname "$WORK")"
if [[ -d "$WORK" ]]; then
  prc_log "reusing bare clone $WORK"
  git -C "$WORK" fetch --quiet origin '+refs/heads/*:refs/heads/*' 2>/dev/null || true
else
  prc_log "bare-cloning $NWO (nothing is checked out)"
  git clone --bare --quiet "$(prc_clone_url "$NWO")" "$WORK" \
    || prc_die "cannot clone $NWO"
fi

if [[ $APPLY -eq 1 ]]; then
  git -C "$WORK" var GIT_AUTHOR_IDENT >/dev/null 2>&1 \
    || prc_die "git has no user.name/user.email configured. Set them, then re-run."
fi

MSG="Remove PolinRider payload

Removes files carrying indicators of the PolinRider supply-chain compromise.
Detected with polinrider-cleaner. This is an additive commit: no history is
rewritten. Rotate any credential that was exposed to an affected clone."

CHANGED=0; SKIPPED=0; FAILED=0
IDX="$WORK/.polinrider-index"

printf '\n'
while read -r ref; do
  [[ -z "$ref" ]] && continue
  old=$(git -C "$WORK" rev-parse --verify --quiet "$ref" 2>/dev/null) || {
    printf '  %-58s gone from remote, skipped\n' "$ref"; SKIPPED=$((SKIPPED+1)); continue; }

  rm -f "$IDX"
  GIT_INDEX_FILE="$IDX" git -C "$WORK" read-tree "$ref" 2>/dev/null || {
    printf '  %-58s cannot read tree, skipped\n' "$ref"; FAILED=$((FAILED+1)); continue; }

  # Remove from the index only. There is no working tree to touch.
  removed=""
  while read -r p; do
    [[ -z "$p" ]] && continue
    if GIT_INDEX_FILE="$IDX" git -C "$WORK" ls-files --error-unmatch -- "$p" >/dev/null 2>&1; then
      GIT_INDEX_FILE="$IDX" git -C "$WORK" rm --cached --quiet -r --ignore-unmatch -- "$p" >/dev/null 2>&1 \
        && removed="${removed}${p} "
    fi
  done <<< "$mapfile_paths"

  if [[ -z "$removed" ]]; then
    printf '  %-58s already clean\n' "$ref"; SKIPPED=$((SKIPPED+1)); continue
  fi

  newtree=$(GIT_INDEX_FILE="$IDX" git -C "$WORK" write-tree 2>/dev/null) || {
    printf '  %-58s cannot write tree, skipped\n' "$ref"; FAILED=$((FAILED+1)); continue; }

  n=$(printf '%s' "$removed" | wc -w | tr -d ' ')
  if [[ $APPLY -eq 0 ]]; then
    printf '  %-58s would remove %s file(s)\n' "$ref" "$n"
    CHANGED=$((CHANGED+1)); continue
  fi

  newcommit=$(printf '%s\n' "$MSG" | git -C "$WORK" commit-tree "$newtree" -p "$old" 2>/dev/null) || {
    printf '  %-58s cannot commit, skipped\n' "$ref"; FAILED=$((FAILED+1)); continue; }
  git -C "$WORK" update-ref "$ref" "$newcommit" "$old" 2>/dev/null || {
    printf '  %-58s cannot update ref, skipped\n' "$ref"; FAILED=$((FAILED+1)); continue; }

  # Plain push. No --force anywhere in this script, by design.
  if git -C "$WORK" push --quiet origin "$ref:$ref" 2>/dev/null; then
    printf '  %-58s removed %s, pushed\n' "$ref" "$n"
    CHANGED=$((CHANGED+1))
  else
    git -C "$WORK" update-ref "$ref" "$old" 2>/dev/null || true
    printf '  %-58s PUSH REJECTED (protected branch?)\n' "$ref"
    FAILED=$((FAILED+1))
  fi
done <<< "$mapfile_refs"
rm -f "$IDX"

printf '\n  branches changed : %s\n' "$CHANGED"
printf '  already clean    : %s\n'   "$SKIPPED"
printf '  failed           : %s\n'   "$FAILED"
if [[ $APPLY -eq 0 ]]; then
  printf '\n  This was a dry run. To do it:\n    %s %s --apply\n' "${PRC_WRAPPER:-$0}" "$NWO"
elif [[ "$FAILED" -gt 0 ]]; then
  printf '\n  Some branches were rejected. A protected branch needs a pull request:\n'
  printf '  open one from a branch this script already cleaned.\n'
fi
[[ "$FAILED" -gt 0 ]] && exit 1
exit 0
