#!/usr/bin/env bash
# install-workflow.sh - vendor the scanner into one of your repositories.
#
# Copies scan-workspace.sh and the indicator set into <repo>/.github/polinrider/,
# and the workflow into <repo>/.github/workflows/polinrider-scan.yml. After this
# the scan runs entirely from code inside your own repository: no marketplace
# action, no network fetch at scan time.
#
# Usage:
#   ./install-workflow.sh /path/to/repo [--force]
#
# It does not commit anything. Review the diff, then commit and push yourself.

set -uo pipefail
SDIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SDIR/.." && pwd)"

TARGET="${1:-}"; FORCE=0
[[ "${2:-}" == "--force" ]] && FORCE=1
[[ -n "$TARGET" && -d "$TARGET" ]] || { echo "usage: install-workflow.sh /path/to/repo [--force]" >&2; exit 2; }
[[ -d "$TARGET/.git" ]] || { echo "not a git repository: $TARGET" >&2; exit 2; }

DEST="$TARGET/.github/polinrider"
WF="$TARGET/.github/workflows/polinrider-scan.yml"

if [[ -e "$WF" && $FORCE -eq 0 ]]; then
  echo "$WF already exists. Re-run with --force to overwrite." >&2
  exit 2
fi

mkdir -p "$DEST/ioc" "$TARGET/.github/workflows"
cp "$SDIR/scan-workspace.sh" "$DEST/scan-workspace.sh"
chmod +x "$DEST/scan-workspace.sh"
cp "$ROOT/ioc/"*.txt "$DEST/ioc/"
cp "$SDIR/polinrider-scan.yml" "$WF"

cat <<EOF
installed into $TARGET

  .github/polinrider/scan-workspace.sh
  .github/polinrider/ioc/*.txt
  .github/workflows/polinrider-scan.yml

Nothing was committed. Next:

  cd "$TARGET"
  .github/polinrider/scan-workspace.sh --path . --all-refs   # confirm it passes now
  git add .github/polinrider .github/workflows/polinrider-scan.yml
  git commit -m "Add PolinRider scan workflow"

Keep the indicator set current by re-running this installer after you update
ioc/*.txt in the cleaner repository.
EOF
