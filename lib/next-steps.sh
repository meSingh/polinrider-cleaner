#!/usr/bin/env bash
# next-steps.sh - turn a finished triage into commands you can actually run.
#
# The scan says what is wrong. This says what to do about it, with real values
# filled in: real repository names, real paths, a real timestamp. Nothing it
# prints contains a placeholder, because a printed placeholder is a command
# that fails when you paste it.
#
# READ-ONLY. It writes NEXT-STEPS.md and three lists into the evidence
# directory and changes nothing else.
#
# Usage:
#   next-steps.sh --triage DIR/triage.json --out DIR --owner NAME --owner-type user|org

set -uo pipefail
# shellcheck source=lib/common.sh
. "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/common.sh"

TRIAGE=""; OUT=""; OWNER=""; KIND=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --triage)     TRIAGE="$2"; shift 2 ;;
    --out)        OUT="$2"; shift 2 ;;
    --owner)      OWNER="$2"; shift 2 ;;
    --owner-type) KIND="$2"; shift 2 ;;
    -h|--help)    sed -n '2,13p' "$0"; exit 0 ;;
    *) prc_die "unknown argument: $1" ;;
  esac
done
[[ -n "$TRIAGE" && -f "$TRIAGE" ]] || prc_die "need --triage <triage.json>"
[[ -n "$OWNER" && -n "$KIND" ]]    || prc_die "need --owner and --owner-type"
OUT="${OUT:-$(dirname "$TRIAGE")}"
prc_need jq

PUSHES="$OUT/pushes.tsv"
REFS="$OUT/affected-refs.tsv"
PATHS="$OUT/affected-paths.tsv"
REPOS="$OUT/affected-repos.txt"
DOC="$OUT/NEXT-STEPS.md"
RECOVERY_DIR="github-account-recovery"
[[ "$KIND" == "org" ]] && RECOVERY_DIR="github-org-recovery"

# --- 1. every (repo, ref, path) that is not our own detection tooling --------
jq -r '
  .[] | select(.verdict=="INFECTED") | . as $e
  | ( [ ($e.ioc_strings[]?     | split(":")[1]),
        ($e.ioc_filenames[]?   | sub(" .*$";"")),
        ($e.font_masquerade[]? | sub(" .*$";"")) ]
      | map(select(. != null and . != "")) | unique )[]
  | [$e.repo, $e.ref, .] | @tsv
' "$TRIAGE" > "$OUT/.all-paths.tsv" || prc_die "cannot read $TRIAGE"

: > "$PATHS"; : > "$REFS"
while IFS=$'\t' read -r repo ref path; do
  [[ -z "${path:-}" ]] && continue
  printf '%s' "$path" | grep -qEi "$PRC_BENIGN_RE" && continue
  printf '%s\t%s\n' "$repo" "$path" >> "$PATHS"
  printf '%s\t%s\n' "$repo" "$ref"  >> "$REFS"
done < "$OUT/.all-paths.tsv"
rm -f "$OUT/.all-paths.tsv"

sort -u -o "$PATHS" "$PATHS"
sort -u -o "$REFS"  "$REFS"
cut -f1 "$REFS" | sort -u > "$REPOS"

N_REPOS=$(awk 'END{print NR}' "$REPOS" 2>/dev/null)
N_REFS=$(awk  'END{print NR}' "$REFS"  2>/dev/null)
if [[ "${N_REFS:-0}" -eq 0 ]]; then
  printf 'No real suspects. Nothing to plan.\n'
  rm -f "$PATHS" "$REFS" "$REPOS"
  exit 0
fi

# --- 2. force-pushed, or committed? The remedy is completely different -------
# A force-push can be undone: the old commit is still in the mirror. A payload
# that was committed normally has no earlier state to go back to, so the fix is
# to remove it and commit that. The push ledger decides which case this is.
T0=""; N_WITH_EVENTS=0
if [[ -f "$PUSHES" ]]; then
  N_WITH_EVENTS=$(awk -F'\t' 'NR==FNR{a[$1];next} FNR>1 && ($1 in a){print $1}' \
                  "$REPOS" "$PUSHES" 2>/dev/null | sort -u | awk 'END{print NR}')
  if [[ "${N_WITH_EVENTS:-0}" -gt 0 ]]; then
    first=$(awk -F'\t' 'NR==FNR{a[$1];next} FNR>1 && ($1 in a){print $6}' \
            "$REPOS" "$PUSHES" 2>/dev/null | sort | head -1)
    if [[ -n "${first:-}" ]]; then
      # Two hours before the earliest push we have on an affected repository.
      T0=$(prc_shift_back_2h "$first" || true)
    fi
  fi
fi

# --- 3. the document --------------------------------------------------------
CLEAN="./$RECOVERY_DIR/clean-repo.sh"
{
  printf '# What to do next\n\n'
  printf 'Generated %s from `%s`.\n\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$TRIAGE"
  printf 'Confirmed: **%s repositories, %s branches**. Every command below is\n' "$N_REPOS" "$N_REFS"
  printf 'complete and runnable as written.\n\n'
  printf -- '---\n\n'

  printf '## Before you touch anything\n\n'
  printf 'Do not open an infected repository in your editor. The payload includes a\n'
  printf '`.vscode/tasks.json` with `"runOn": "folderOpen"`, which runs a command the\n'
  printf 'moment VS Code opens the folder. The cleanup below works in a scratch clone\n'
  printf 'under your evidence directory, away from any editor.\n\n'
  printf 'Do not `git pull` in an existing clone of an affected repository either.\n\n'

  printf '### 1. Check this machine\n\n'
  printf '```bash\n./polinrider.sh --machine\n```\n\n'
  printf 'If it reports a confirmed hit, do everything else from a different computer.\n\n'

  printf '### 2. Rotate your credentials\n\n'
  printf 'From a machine that came back clean: personal access tokens, SSH keys,\n'
  printf 'signing keys, OAuth grants, then a password change with sign-out of all\n'
  printf 'sessions. Full list in README.md, "Rotate every credential".\n\n'
  printf -- '---\n\n'

  printf '## 3. How it got there\n\n'
  if [[ -n "$T0" ]]; then
    printf 'The push ledger has events for %s of the %s affected repositories, so a\n' "$N_WITH_EVENTS" "$N_REPOS"
    printf 'force-push is possible. Check that first, because if the branches were\n'
    printf 'force-pushed you can move them back instead of editing files.\n\n'
    printf 'T0 below is two hours before the earliest push recorded on an affected\n'
    printf 'repository. It is already filled in.\n\n'
    printf '```bash\n'
    printf './%s/sweep.sh --%s %s --since %s --out %s\n' "$RECOVERY_DIR" "$KIND" "$OWNER" "$T0" "$OUT"
    printf '```\n\n'
    printf 'Read `%s/sweep.tsv`. If it shows pushes nobody on your team claims,\n' "$OUT"
    printf 'restore rather than clean:\n\n'
    printf '```bash\n'
    printf './%s/restore.sh --sweep %s/sweep.tsv --mirrors %s --since %s\n' "$RECOVERY_DIR" "$OUT" "$OUT" "$T0"
    printf './%s/preflight.sh --plan %s/restore-plan.tsv\n' "$RECOVERY_DIR" "$OUT"
    printf '```\n\n'
    printf 'Add `--apply` to the restore once preflight is clean.\n\n'
    printf 'If the sweep shows nothing unexpected, the payload was committed normally.\n'
    printf 'Use step 4.\n\n'
  else
    printf 'No push events survive for any of the %s affected repositories.\n\n' "$N_REPOS"
    printf 'The GitHub events API keeps roughly 300 events per repository for about\n'
    printf '90 days. Nothing is left for these, which means one of two things, and\n'
    printf 'both lead to the same fix:\n\n'
    printf '%s\n' '- the payload was committed normally rather than force-pushed, or'
    printf '%s\n\n' '- it was force-pushed longer ago than the API remembers.'
    printf 'Either way there is **no earlier state to restore to**, so `restore.sh`\n'
    printf 'has nothing to work from. Do not run it. Remove the files and commit\n'
    printf 'that removal: step 4.\n\n'
  fi
  printf -- '---\n\n'

  printf '## 4. Remove the payload\n\n'
  printf '`clean-repo.sh` clones into your evidence directory, removes the flagged\n'
  printf 'paths from every affected branch, commits, and pushes. It is a normal\n'
  printf 'commit on top, not a force-push, so nothing is rewritten and you can\n'
  printf 'revert it. **It is a dry run unless you pass `--apply`.**\n\n'
  printf 'Start with one repository and read what it says:\n\n'
  printf '```bash\n%s %s\n```\n\n' "$CLEAN" "$(head -1 "$REPOS")"
  printf 'Then, when the plan looks right:\n\n'
  printf '```bash\n%s %s --apply\n```\n\n' "$CLEAN" "$(head -1 "$REPOS")"
  printf 'All %s, one at a time, dry run first:\n\n' "$N_REPOS"
  printf '```bash\nwhile read -r repo; do %s "$repo"; done < %s\n```\n\n' "$CLEAN" "$REPOS"
  printf 'And to apply, once you have read the dry run:\n\n'
  printf '```bash\nwhile read -r repo; do %s "$repo" --apply; done < %s\n```\n\n' "$CLEAN" "$REPOS"
  printf -- '---\n\n'

  printf '## 5. Confirm\n\n'
  printf '```bash\n./polinrider.sh --%s %s --out %s-post\n```\n\n' "$KIND" "$OWNER" "$OUT"
  printf 'Then delete every local clone of an affected repository and clone fresh.\n'
  printf 'An old clone still holds the payload and pushing from it puts it back.\n\n'
  printf -- '---\n\n'

  printf '## What was found\n\n'
  printf '| Repository | Branches | Paths |\n|---|---|---|\n'
  while read -r repo; do
    nrefs=$(awk -F'\t' -v r="$repo" '$1==r' "$REFS" | awk 'END{print NR}')
    plist=$(awk -F'\t' -v r="$repo" '$1==r {print "`" $2 "`"}' "$PATHS" | sort -u | paste -sd' ' -)
    printf '| `%s` | %s | %s |\n' "$repo" "$nrefs" "$plist"
  done < "$REPOS"
  printf '\nBranch-level detail: `%s`\nMatched content: `%s/triage.txt`\n' "$REFS" "$OUT"
} > "$DOC"

# --- 4. the short version, on screen ----------------------------------------
printf '\n'
printf '  %s repositories, %s branches. Written to:\n' "$N_REPOS" "$N_REFS"
printf '    %s\n\n' "$DOC"
if [[ -n "$T0" ]]; then
  printf '  Push events survive for %s repo(s), so check for a force-push first:\n' "$N_WITH_EVENTS"
  printf '    ./%s/sweep.sh --%s %s --since %s --out %s\n\n' "$RECOVERY_DIR" "$KIND" "$OWNER" "$T0" "$OUT"
else
  printf '  No push events survive for any affected repository, so there is no\n'
  printf '  earlier state to restore to. Do not run sweep.sh or restore.sh.\n'
  printf '  The payload was committed, so removing it is the fix.\n\n'
fi
printf '  Clean one repository, dry run, then for real:\n'
printf '    %s %s\n'         "$CLEAN" "$(head -1 "$REPOS")"
printf '    %s %s --apply\n' "$CLEAN" "$(head -1 "$REPOS")"
exit 0
