#!/usr/bin/env bash
# scan-workspace.sh - scan a checked-out working tree, and optionally every ref in
#                     its history, for PolinRider indicators.
#
# Self-contained. Needs bash, git and standard POSIX tools. No Node, no Python,
# no third-party action. Vendor this file and the ioc/ directory into your own
# repository so a supply-chain compromise of someone else's tooling cannot
# compromise your scan.
#
# Usage:
#   ./scan-workspace.sh [--path DIR] [--all-refs] [--fail-on LEVEL] [--exclude RE]...
#
# Options:
#   --path DIR        directory to scan. Default: current directory
#   --all-refs        also scan every git ref, not just the working tree.
#                     Catches a payload that was committed and then reverted.
#   --fail-on LEVEL   infected (default) | review | never
#   --exclude RE      extended regex of paths to skip. Repeatable.
#   --scan-docs       also scan .md files. Skipped by default: documentation about
#                     this campaign contains the indicator strings it describes,
#                     and markdown does not execute.
#   --ioc DIR         indicator directory. Default: ioc/ beside or above this script
#
# Exit codes: 0 clean, 1 review items only (with --fail-on review), 2 infected,
#             3 could not scan. 3 is never a verdict about the code you pointed it at.

set -uo pipefail

SDIR="$(cd "$(dirname "$0")" && pwd)"
PATH_ARG="."; ALL_REFS=0; FAIL_ON="infected"; IOC_DIR=""; SCAN_DOCS=0; EXTRA_EXCLUDES=""
EXCLUDES='(^|/)\.git/|(^|/)node_modules/|(^|/)\.github/polinrider/|(^|/)ioc/|(^|/)\.github/workflows/[^/]*polinrider[^/]*\.ya?ml$'

while [[ $# -gt 0 ]]; do
  case "$1" in
    --path)     PATH_ARG="$2"; shift 2 ;;
    --all-refs) ALL_REFS=1; shift ;;
    --fail-on)  FAIL_ON="$2"; shift 2 ;;
    --exclude)  EXTRA_EXCLUDES="$EXTRA_EXCLUDES|$2"; shift 2 ;;
    --scan-docs) SCAN_DOCS=1; shift ;;
    --ioc)      IOC_DIR="$2"; shift 2 ;;
    -h|--help)  sed -n '2,26p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 3 ;;
  esac
done

# Markdown carries indicator strings whenever someone documents this campaign,
# and it cannot execute. Skipped unless asked for.
[[ $SCAN_DOCS -eq 0 ]] && EXCLUDES="$EXCLUDES|\.md$"
EXCLUDES="$EXCLUDES$EXTRA_EXCLUDES"

if [[ -z "$IOC_DIR" ]]; then
  if   [[ -f "$SDIR/ioc/strong.txt"    ]]; then IOC_DIR="$SDIR/ioc"
  elif [[ -f "$SDIR/../ioc/strong.txt" ]]; then IOC_DIR="$SDIR/../ioc"
  else echo "cannot find ioc/strong.txt. Pass --ioc DIR" >&2; exit 3; fi
fi
[[ -d "$PATH_ARG" ]] || { echo "no such directory: $PATH_ARG" >&2; exit 3; }

WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
strip() { sed -e 's/[[:space:]]*$//' -e '/^#/d' -e '/^$/d' "$@"; }
strip "$IOC_DIR/strong.txt" "$IOC_DIR/bad-packages.txt" > "$WORK/strong"
strip "$IOC_DIR/weak.txt"       > "$WORK/weak"
strip "$IOC_DIR/filenames.txt"  > "$WORK/filenames"
[[ -s "$WORK/strong" ]] || { echo "indicator set is empty" >&2; exit 3; }

# Obfuscation tells used to qualify a "content after module end" finding.
TAIL_TELL='eval\(|new Function\(|Buffer\.from\(|child_process|atob\(|fromCharCode|\\x[0-9a-fA-F]{2}\\x[0-9a-fA-F]{2}|require\([^)]*(child_process|https?|net|dns)'

cd "$PATH_ARG" || exit 3
HITS=0; REVIEWS=0
# Findings quote file content and paths, both of which an attacker controls.
# Strip control characters so a crafted file cannot emit terminal escape
# sequences into the operator's console or into a CI log.
clean() { LC_ALL=C tr -d '\000-\010\013\014\016-\037\177'; }
note()  { printf '%s\n' "$*"; }
hit()   { local m; m="$(printf '%s' "$*" | clean)"; HITS=$((HITS+1))
          printf 'INFECTED  %s\n' "$m"
          [[ "${GITHUB_ACTIONS:-}" == "true" ]] && printf '::error title=PolinRider::%s\n' "$m"; }
review(){ local m; m="$(printf '%s' "$*" | clean)"; REVIEWS=$((REVIEWS+1))
          printf 'review    %s\n' "$m"
          [[ "${GITHUB_ACTIONS:-}" == "true" ]] && printf '::warning title=PolinRider review::%s\n' "$m"; }

# ---------------------------------------------------------------------------
# Working tree
# ---------------------------------------------------------------------------
note "== working tree: $(pwd) =="

FILES="$WORK/files"
find . -type f 2>/dev/null | sed 's|^\./||' | grep -Ev "$EXCLUDES" > "$FILES"
note "files in scope: $(grep -c . "$FILES")"

# One grep over the whole file list, not one grep per file. On a 3,000 file tree
# the per-file loop cost 24 seconds; this costs under one. /dev/null is passed so
# grep always prefixes the filename, even when xargs hands it a single file.
scan_batch() {
  tr '\n' '\0' < "$FILES" | xargs -0 grep -I -n -F -f "$1" /dev/null 2>/dev/null \
    | awk -F: '!seen[$1]++'
}

# 1. indicator strings
while IFS= read -r m; do
  [[ -z "$m" ]] && continue
  hit "$(printf '%s' "$m" | cut -c1-200)"
done < <(scan_batch "$WORK/strong")

# 2. weak signals
while IFS= read -r m; do
  [[ -z "$m" ]] && continue
  review "$(printf '%s' "$m" | cut -c1-200)"
done < <(scan_batch "$WORK/weak")

# 3. indicator filenames
while IFS= read -r f; do
  [[ -n "$f" ]] && hit "$f: filename is a known propagation artifact"
done < <(grep -E -f "$WORK/filenames" "$FILES" 2>/dev/null)

# 4. tasks.json that runs on folder open
while IFS= read -r f; do
  [[ -z "$f" ]] && continue
  grep -qF 'folderOpen' "$f" 2>/dev/null || continue
  if grep -qI -F -f "$WORK/strong" "$f" 2>/dev/null; then
    hit "$f: task runs on folder open and contains an indicator"
  else
    review "$f: task runs on folder open, verify the command it runs"
  fi
done < <(grep -E '(^|/)\.vscode/tasks\.json$' "$FILES" 2>/dev/null)

# 5. font masquerade
while IFS= read -r f; do
  [[ -s "$f" ]] || continue
  MAGIC="$(head -c 4 "$f" 2>/dev/null)"
  case "$MAGIC" in
    wOFF|wOF2|vers) ;;
    *) hit "$f: font file is not a font (first bytes: $(head -c 4 "$f" 2>/dev/null | od -An -tx1 | tr -s ' ' | sed 's/^ *//;s/ *$//'))" ;;
  esac
done < <(grep -Ei '\.woff2?$' "$FILES" 2>/dev/null | grep -Ev '(^|/)__MACOSX/|(^|/)\._')

# 6. build config with code after the module end
while IFS= read -r f; do
  [[ -z "$f" ]] && continue
  TOTAL="$(grep -c '' "$f" 2>/dev/null || echo 0)"
  END="$(grep -n -E '^(export default|module\.exports)' "$f" 2>/dev/null | tail -1 | cut -d: -f1)"
  if [[ -n "$END" && "$TOTAL" -gt $(( END + 15 )) ]]; then
    # Content after the module end is normal in flat configs (export default [ ... ]).
    # It is only a signal when that content also looks like a payload.
    TAIL="$(tail -n +$(( END + 1 )) "$f" 2>/dev/null)"
    if printf '%s' "$TAIL" | grep -qE "$TAIL_TELL" \
       || printf '%s' "$TAIL" | awk 'length($0) > 500 {found=1} END{exit !found}'; then
      review "$f: $TOTAL lines, module ends at $END, and the remainder looks like a payload"
    fi
  fi
  awk 'length($0) > 4000 {found=1} END{exit !found}' "$f" 2>/dev/null && \
    review "$f: line longer than 4000 characters, an obfuscation tell"
done < <(grep -E '((postcss|tailwind|eslint|vite|next|rollup|webpack|babel|gridsome|vue)\.config\.(js|mjs|cjs|ts)|truffle\.js)$' "$FILES" 2>/dev/null)

# ---------------------------------------------------------------------------
# History
# ---------------------------------------------------------------------------
if [[ $ALL_REFS -eq 1 ]]; then
  if ! git rev-parse --git-dir >/dev/null 2>&1; then
    note "not a git repository, --all-refs skipped"
  else
    note ""
    note "== all refs =="
    while IFS= read -r ref; do
      [[ -z "$ref" ]] && continue
      shown=0
      while IFS= read -r line; do
        [[ -z "$line" ]] && continue
        # git grep prints "<ref>:<path>:<line>:<content>". The exclusion regex
        # matches paths, so pull the path out before testing it - testing the
        # whole line silently excludes nothing.
        rest="${line#*:}"
        gpath="${rest%%:*}"
        printf '%s' "$gpath" | grep -Eq "$EXCLUDES" && continue
        [[ $shown -ge 5 ]] && continue
        shown=$(( shown + 1 ))
        hit "$(printf '%s' "$line" | cut -c1-200)"
      done < <(git grep -I -n -F -f "$WORK/strong" "$ref" 2>/dev/null | head -200)
    done < <(git for-each-ref --format='%(refname)' refs/ 2>/dev/null)
  fi
fi

# ---------------------------------------------------------------------------
note ""
note "========================================="
note " PolinRider workspace scan"
note "   infected findings : $HITS"
note "   review findings   : $REVIEWS"
note "========================================="
if [[ $HITS -gt 0 ]]; then
  note " A finding here means a file in this repository matches a known indicator."
  note " If this file is your own detection tooling, add its path with --exclude."
  note " Otherwise: do not merge, do not open this branch in an editor, and follow"
  note " the recovery steps in the PolinRider cleaner README."
fi
case "$FAIL_ON" in
  never)    exit 0 ;;
  review)   [[ $HITS -gt 0 ]] && exit 2; [[ $REVIEWS -gt 0 ]] && exit 1; exit 0 ;;
  infected) [[ $HITS -gt 0 ]] && exit 2; exit 0 ;;
  *) echo "unknown --fail-on value: $FAIL_ON" >&2; exit 3 ;;
esac
