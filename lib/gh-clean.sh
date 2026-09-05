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
#   gh-clean.sh OWNER/REPO --rewrite [--apply] [--out DIR]
#
# --rewrite removes the paths from every commit in the history instead of adding
# a commit that deletes them. Use it when leaving the payload reachable is not
# acceptable: anyone can check out an older commit and get a live
# .vscode/tasks.json that runs on folder open.
#
# It rewrites every commit, so every SHA changes and every ref needs a
# force-push. Read the two warnings it prints before using --apply.
#
# Reads affected-paths.tsv and affected-refs.tsv from the evidence directory,
# both written by next-steps.sh.

set -uo pipefail
# shellcheck source=lib/common.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

NWO=""; APPLY=0; OUT=""; REWRITE=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply)   APPLY=1; shift ;;
    --rewrite) REWRITE=1; shift ;;
    --out)     OUT="$2"; shift 2 ;;
    -h|--help) sed -n '2,29p' "$0"; exit 0 ;;
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


# --- history rewriting ------------------------------------------------------
# git filter-repo is a hundred times faster and is the tool git itself points
# you at, but it is Python and this repository does not otherwise need an
# interpreter. So: use it when it is already installed, fall back to
# filter-branch when it is not. Nobody is asked to install anything.
prc_rewrite_backend() {
  case "${PRC_REWRITE_BACKEND:-}" in
    filter-repo|filter-branch) printf '%s\n' "$PRC_REWRITE_BACKEND"; return 0 ;;
  esac
  if git filter-repo --version >/dev/null 2>&1; then printf 'filter-repo\n'
  else printf 'filter-branch\n'; fi
}

# prc_shquote <string> - single-quote for a string that a shell will evaluate.
prc_shquote() { printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"; }

# prc_rewrite <bare-repo> <backend> <path>... - drop the paths from all history.
prc_rewrite() {
  local work="$1" backend="$2"; shift 2
  case "$backend" in
    filter-repo)
      local args=() p
      for p in "$@"; do args+=(--path "$p"); done
      # --force because the mirror is not a fresh clone. filter-repo drops the
      # origin remote afterwards on purpose; the caller puts it back.
      git -C "$work" filter-repo --invert-paths "${args[@]}" --force
      ;;
    filter-branch)
      local rm_args="" p
      for p in "$@"; do rm_args="$rm_args $(prc_shquote "$p")"; done
      FILTER_BRANCH_SQUELCH_WARNING=1 git -C "$work" filter-branch -f \
        --index-filter "git rm --cached --ignore-unmatch -r --$rm_args" \
        --prune-empty --tag-name-filter cat -- --all
      ;;
    *) prc_die "unknown rewrite backend: $backend" ;;
  esac
}

# --- rewrite mode -----------------------------------------------------------
if [[ $REWRITE -eq 1 ]]; then
  BACKEND="$(prc_rewrite_backend)"
  PATHS_ARR=()
  while IFS= read -r p; do [[ -n "$p" ]] && PATHS_ARR+=("$p"); done <<< "$mapfile_paths"

  touched=$(git -C "$WORK" rev-list --branches --tags --count -- "${PATHS_ARR[@]}" 2>/dev/null || echo 0)
  total=$(git -C "$WORK" rev-list --branches --tags --count 2>/dev/null || echo 0)
  allrefs=$(git -C "$WORK" for-each-ref --format='%(refname)' 'refs/heads/*' 'refs/tags/*' | awk 'END{print NR}')

  printf '\n  Rewrite mode\n'
  printf '    backend        : %s%s\n' "$BACKEND" \
    "$([[ "$BACKEND" == filter-branch ]] && printf ' (git filter-repo is not installed; this is slower)' || printf ' (fast path)')"
  printf '    commits touching those paths : %s\n' "$touched"
  printf '    commits in all history       : %s   all of these get a new SHA\n' "$total"
  printf '    refs to force-push           : %s\n' "$allrefs"

  if [[ $APPLY -eq 0 ]]; then
    cat <<'EOF'

  DRY RUN. Nothing is rewritten and nothing is pushed.

  Before you use --apply, two things that are true and easy to miss:

  1. This does not remove anything from GitHub. GitHub keeps unreachable
     objects and still serves them by SHA. Anyone holding an old commit id can
     fetch it after the rewrite. To actually purge them you have to ask GitHub
     Support to run gc on the repository. Forks keep their own copies
     regardless.
  2. Every commit id in the repository changes. Existing clones diverge, open
     pull requests may close, and any link to a commit stops resolving.

  What this does achieve: the payload leaves the reachable history, so it is
  gone from git log, from git blame, and from every clone made after this.
EOF
    printf '\n  To do it:\n    %s %s --rewrite --apply\n' "${PRC_WRAPPER:-$0}" "$NWO"
    exit 0
  fi

  # Keep the pre-rewrite state addressable until the operator is satisfied.
  STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
  git -C "$WORK" for-each-ref --format='%(objectname) %(refname)' 'refs/heads/*' \
    | while read -r sha ref; do
        git -C "$WORK" update-ref "refs/polinrider/pre-rewrite/$STAMP/${ref#refs/heads/}" "$sha" 2>/dev/null || true
      done
  prc_log "pre-rewrite refs saved under refs/polinrider/pre-rewrite/$STAMP"

  ORIGIN_URL="$(git -C "$WORK" remote get-url origin 2>/dev/null || prc_clone_url "$NWO")"
  prc_log "rewriting with $BACKEND, this can take a while"
  if ! prc_rewrite "$WORK" "$BACKEND" "${PATHS_ARR[@]}"; then
    prc_die "rewrite failed. The mirror is unchanged on the remote; nothing was pushed."
  fi

  # Only the refs that get pushed. refs/original/* and the pre-rewrite refs are
  # deliberate local backups and still contain the payload by design.
  left=$(git -C "$WORK" rev-list --branches --tags --count -- "${PATHS_ARR[@]}" 2>/dev/null || echo 0)
  if [[ "${left:-0}" -ne 0 ]]; then
    prc_die "rewrite left $left commit(s) reachable from a branch or tag. Not pushing."
  fi
  printf '  payload removed from all reachable history\n'

  # filter-repo removes origin deliberately so a rewrite cannot be pushed by
  # accident. Put it back, having decided to push on purpose.
  git -C "$WORK" remote get-url origin >/dev/null 2>&1 \
    || git -C "$WORK" remote add origin "$ORIGIN_URL"

  prc_log "force-pushing every branch and tag"
  git -C "$WORK" push --force origin 'refs/heads/*:refs/heads/*' 2>&1 | sed 's/^/    /'
  git -C "$WORK" push --force origin 'refs/tags/*:refs/tags/*'   2>&1 | sed 's/^/    /'

  # Do not trust the push output. Compare what is on the remote against local.
  BAD=0
  while read -r lsha lref; do
    rsha=$(git -C "$WORK" ls-remote origin "$lref" 2>/dev/null | awk '{print $1}')
    if [[ "$rsha" != "$lsha" ]]; then
      printf '    NOT UPDATED  %s  (protected branch, or the push was rejected)\n' "$lref"
      BAD=$((BAD+1))
    fi
  done < <(git -C "$WORK" for-each-ref --format='%(objectname) %(refname)' 'refs/heads/*')

  printf '\n  refs verified on the remote : %s\n' "$(( allrefs - BAD ))"
  printf '  refs not updated           : %s\n' "$BAD"
  printf '\n  The old commits are still fetchable from GitHub by SHA. To have them\n'
  printf '  removed, contact GitHub Support and ask them to gc this repository.\n'
  printf '  Pre-rewrite refs are kept locally under refs/polinrider/pre-rewrite/%s\n' "$STAMP"
  [[ "$BAD" -gt 0 ]] && exit 1
  exit 0
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
