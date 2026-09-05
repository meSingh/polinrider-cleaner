#!/usr/bin/env bash
# install-workflow.sh - vendor the scanner into one of your repositories.
#
# Copies scan-workspace.sh and the indicator set into <repo>/.github/polinrider/,
# and the workflow into <repo>/.github/workflows/polinrider-scan.yml. After this
# the scan runs entirely from code inside your own repository: no marketplace
# action, no network fetch at scan time.
#
# Usage:
#   ./install-workflow.sh /path/to/repo [--dry-run] [--force]
#
# Options:
#   --dry-run   list every file it would write, and write nothing
#   --force     overwrite an existing workflow
#
# It never commits. Review the diff, then commit and push yourself.

set -uo pipefail
SDIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SDIR/.." && pwd)"

TARGET=""; FORCE=0; DRY=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --force)   FORCE=1; shift ;;
    --dry-run) DRY=1; shift ;;
    -h|--help) sed -n '2,14p' "$0"; exit 0 ;;
    -*) echo "unknown argument: $1" >&2; exit 2 ;;
    *)  TARGET="$1"; shift ;;
  esac
done

[[ -n "$TARGET" && -d "$TARGET" ]] || { echo "usage: install-workflow.sh /path/to/repo [--dry-run] [--force]" >&2; exit 2; }
[[ -d "$TARGET/.git" ]] || { echo "not a git repository: $TARGET" >&2; exit 2; }

DEST="$TARGET/.github/polinrider"
WF="$TARGET/.github/workflows/polinrider-scan.yml"

if [[ -e "$WF" && $FORCE -eq 0 && $DRY -eq 0 ]]; then
  echo "$WF already exists. Re-run with --force to overwrite." >&2
  exit 2
fi

# --- what will be written ---------------------------------------------------
IOC_FILES=""
for f in "$ROOT/ioc/"*.txt; do IOC_FILES="$IOC_FILES $(basename "$f")"; done

if [[ $DRY -eq 1 ]]; then
  echo "DRY RUN. Nothing will be written."
  echo
fi
printf 'Target repository: %s\n\n' "$TARGET"
echo "Files:"
printf '  %-52s %s\n' ".github/workflows/polinrider-scan.yml" "the workflow itself"
printf '  %-52s %s\n' ".github/polinrider/scan-workspace.sh" "the scanner, executable"
for f in $IOC_FILES; do
  printf '  %-52s %s\n' ".github/polinrider/ioc/$f" "indicator data"
done
BYTES=$(cat "$SDIR/scan-workspace.sh" "$SDIR/polinrider-scan.yml" "$ROOT/ioc/"*.txt | wc -c | tr -d ' ')
N_IOC=$(printf '%s\n' $IOC_FILES | grep -c .)
printf '\nTotal: %s files, about %s KB. No dependencies are added.\n' \
  "$(( N_IOC + 2 ))" "$(( BYTES / 1024 + 2 ))"

if [[ -e "$WF" ]]; then
  echo
  if [[ $FORCE -eq 1 ]]; then echo "NOTE: an existing workflow at that path will be overwritten."
  else echo "NOTE: a workflow already exists at that path. --force would overwrite it."; fi
fi

if [[ $DRY -eq 1 ]]; then
  cat <<'EOF'

What the workflow will then do, on every push and pull request, plus weekly:

  1. Check out the repository with full history and no credentials left behind.
  2. Scan the working tree and every git ref for the indicator set.
  3. Annotate findings on the run. An INFECTED finding fails the job; a review
     finding does not.

Run again without --dry-run to write the files.
EOF
  exit 0
fi

mkdir -p "$DEST/ioc" "$TARGET/.github/workflows"
cp "$SDIR/scan-workspace.sh" "$DEST/scan-workspace.sh"
chmod +x "$DEST/scan-workspace.sh"
cp "$ROOT/ioc/"*.txt "$DEST/ioc/"
cp "$SDIR/polinrider-scan.yml" "$WF"

cat <<EOF

Written. Nothing was committed.

Check it passes before you commit, so any false positive shows up here rather
than in CI:

  cd "$TARGET"
  .github/polinrider/scan-workspace.sh --path . --all-refs

Then:

  git add .github/polinrider .github/workflows/polinrider-scan.yml
  git commit -m "Add PolinRider scan workflow"

Keep the indicator set current by re-running this installer whenever ioc/*.txt
changes in the cleaner repository.
EOF
