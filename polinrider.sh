#!/usr/bin/env bash
# polinrider.sh - the one command to start with.
#
# Works out what you need to scan, picks the right tool for the machine you are
# on, and runs it. Everything it runs is read-only: it reports and then tells you
# the exact next command. It never changes anything.
#
# Usage:
#   ./polinrider.sh                     ask, then scan
#   ./polinrider.sh --machine           scan this computer
#   ./polinrider.sh --org ACME          scan a GitHub organization
#   ./polinrider.sh --user LOGIN        scan a personal GitHub account
#   ./polinrider.sh --path DIR          scan one folder or repository
#   ./polinrider.sh --all --org ACME    everything, in the order that works
#
# Options:
#   --roots "A B"   code directories for the machine scan
#   --out DIR       where evidence goes. Default: ./evidence
#   --yes           never prompt. For scripts and AI agents
#   -h, --help      this text
#
# Exit codes: 0 nothing found, 1 review items only, 2 something confirmed.

set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"

MODE=""; ORG=""; USR=""; SCANPATH=""; OUT="./evidence"; ROOTS=""; ASSUME_YES=0; DO_ALL=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --machine) MODE="machine"; shift ;;
    --org)     MODE="org";  ORG="$2"; shift 2 ;;
    --user)    MODE="user"; USR="$2"; shift 2 ;;
    --path)    MODE="path"; SCANPATH="$2"; shift 2 ;;
    --all)     DO_ALL=1; shift ;;
    --roots)   ROOTS="$2"; shift 2 ;;
    --out)     OUT="$2"; shift 2 ;;
    --yes|-y)  ASSUME_YES=1; shift ;;
    -h|--help) sed -n '2,27p' "$0"; exit 0 ;;
    *) printf 'unknown argument: %s\nTry --help\n' "$1" >&2; exit 2 ;;
  esac
done

# --- presentation -----------------------------------------------------------
if [[ -t 1 && -z "${NO_COLOR:-}" ]]; then
  B=$'\033[1m'; DIM=$'\033[2m'; R=$'\033[31m'; Y=$'\033[33m'; G=$'\033[32m'; C=$'\033[36m'; X=$'\033[0m'
else
  B=""; DIM=""; R=""; Y=""; G=""; C=""; X=""
fi
say()  { printf '%s\n' "$*"; }
head2(){ printf '\n%s%s%s\n' "$B" "$*" "$X"; }
rule() { printf '%s%s%s\n' "$DIM" "────────────────────────────────────────────────────────────" "$X"; }
step() { printf '\n%s▸ %s%s\n' "$C" "$*" "$X"; }
warn() { printf '%s! %s%s\n' "$Y" "$*" "$X"; }
bad()  { printf '%s✗ %s%s\n' "$R" "$*" "$X"; }
good() { printf '%s✓ %s%s\n' "$G" "$*" "$X"; }

ask() {  # ask <prompt> <default y|n>  -> 0 for yes
  local prompt="$1" def="$2" reply
  [[ $ASSUME_YES -eq 1 ]] && return 0
  if [[ ! -t 0 ]]; then [[ "$def" == "y" ]] && return 0 || return 1; fi
  if [[ "$def" == "y" ]]; then printf '%s [Y/n] ' "$prompt"; else printf '%s [y/N] ' "$prompt"; fi
  read -r reply
  case "$reply" in
    [yY]*) return 0 ;;
    [nN]*) return 1 ;;
    "")    [[ "$def" == "y" ]] && return 0 || return 1 ;;
    *)     [[ "$def" == "y" ]] && return 0 || return 1 ;;
  esac
}

# --- what machine is this ---------------------------------------------------
OSNAME="$(uname -s 2>/dev/null || echo unknown)"
case "$OSNAME" in
  Darwin)                      OS="macOS";   LOCAL_TOOL="$HERE/machine-cleanup/check-macos.sh" ;;
  Linux)                       OS="Linux";   LOCAL_TOOL="$HERE/machine-cleanup/check-linux.sh" ;;
  MINGW*|MSYS*|CYGWIN*|Windows_NT) OS="Windows"; LOCAL_TOOL="" ;;
  *)                           OS="$OSNAME"; LOCAL_TOOL="$HERE/machine-cleanup/check-linux.sh" ;;
esac

default_roots() {
  local d out=""
  if [[ "$OS" == "macOS" ]]; then
    for d in "$HOME/Sites" "$HOME/Projects" "$HOME/code" "$HOME/dev" "$HOME/Documents"; do
      [[ -d "$d" ]] && out="$out $d"
    done
  else
    for d in "$HOME/src" "$HOME/code" "$HOME/dev" "$HOME/projects" "$HOME/work" "$HOME/git"; do
      [[ -d "$d" ]] && out="$out $d"
    done
  fi
  printf '%s' "${out# }"
}

have() { command -v "$1" >/dev/null 2>&1; }

check_deps() {  # check_deps <need-gh 0|1>
  local missing=0
  have git || { bad "git is not installed"; missing=1; }
  if [[ "$1" -eq 1 ]]; then
    have gh || { bad "gh (GitHub CLI) is not installed - https://cli.github.com"; missing=1; }
    have jq || { bad "jq is not installed"; missing=1; }
    if have gh && ! gh auth status >/dev/null 2>&1; then
      bad "gh is installed but not signed in. Run: gh auth login"; missing=1
    fi
  fi
  return $missing
}

WORST=0
note_rc() { [[ "$1" -gt "$WORST" ]] && WORST="$1"; return 0; }

# --- the scans --------------------------------------------------------------
scan_machine() {
  step "Scanning this machine ($OS)"
  if [[ "$OS" == "Windows" ]]; then
    warn "On Windows the machine check is PowerShell, which this shell cannot run for you."
    say  "Run this instead, from the repository root:"
    say  ""
    say  "    powershell -ExecutionPolicy Bypass -File .\\machine-cleanup\\check-windows.ps1 -Roots C:\\your\\code"
    say  ""
    return 0
  fi
  check_deps 0 || return 2
  local r; r="${ROOTS:-$(default_roots)}"
  if [[ -z "$r" ]]; then
    warn "No common code directory found under \$HOME."
    say  "Re-run with the real ones, for example:  --roots \"\$HOME/work \$HOME/src\""
    return 1
  fi
  say "${DIM}roots: $r${X}"
  say "${DIM}this reads only; the first run takes a few minutes${X}"
  # shellcheck disable=SC2086  # roots are separate arguments on purpose
  "$LOCAL_TOOL" $r
  local rc=$?; note_rc $rc; return $rc
}

scan_path() {
  step "Scanning $SCANPATH"
  check_deps 0 || return 2
  local extra=""
  [[ -d "$SCANPATH/.git" ]] && extra="--all-refs"
  # shellcheck disable=SC2086  # extra is one optional flag
  "$HERE/ci/scan-workspace.sh" --path "$SCANPATH" $extra
  local rc=$?; note_rc $rc; return $rc
}

scan_github() {  # scan_github <org|user> <name>
  local kind="$1" name="$2" dir
  if [[ "$kind" == "org" ]]; then dir="$HERE/github-org-recovery"; else dir="$HERE/github-account-recovery"; fi
  step "Scanning the $([[ "$kind" == org ]] && echo "organization" || echo "account") $name"
  check_deps 1 || return 2
  say "${DIM}mirror-cloning every repository into $OUT - this can take a while${X}"
  if [[ "$kind" == "org" ]]; then
    "$dir/scan.sh" --org "$name" --out "$OUT" || return 2
  else
    "$dir/scan.sh" --user "$name" --out "$OUT" || return 2
  fi
  step "Separating real findings from your own detection tooling"
  "$dir/triage-filter.sh" "$OUT/triage.json"
  local rc=$?; note_rc $rc; return $rc
}

# --- interactive ------------------------------------------------------------
choose() {
  head2 "PolinRider cleaner"
  say "This machine looks like ${B}${OS}${X}."
  say ""
  say "What do you want to check?"
  say ""
  say "  ${B}1${X}  This computer            ${DIM}files, extensions, persistence, an installed implant${X}"
  say "  ${B}2${X}  A GitHub organization    ${DIM}every repository and branch${X}"
  say "  ${B}3${X}  My GitHub account        ${DIM}every repository you own${X}"
  say "  ${B}4${X}  One folder or repo       ${DIM}quickest check${X}"
  say "  ${B}5${X}  All of it, in order      ${DIM}recommended if you think you were hit${X}"
  say ""
  local pick
  printf 'Choose 1-5: '
  # Reading rather than requiring a tty, so answers can be piped in. On an empty
  # or closed stdin the read fails and the caller prints usage instead of hanging.
  read -r pick || return 1
  [[ -z "$pick" ]] && return 1
  case "$pick" in
    1) MODE="machine" ;;
    2) MODE="org";  printf 'Organization name: '; read -r ORG || return 1; [[ -n "$ORG" ]] || return 1 ;;
    3) MODE="user"; printf 'Your GitHub username: '; read -r USR || return 1; [[ -n "$USR" ]] || return 1 ;;
    4) MODE="path"; printf 'Path to the folder: '; read -r SCANPATH || return 1; [[ -n "$SCANPATH" ]] || return 1 ;;
    5) DO_ALL=1; MODE="machine"
       printf 'GitHub organization (leave blank to skip): '; read -r ORG
       if [[ -z "$ORG" ]]; then printf 'Your GitHub username (leave blank to skip): '; read -r USR; fi ;;
    *) bad "Not one of the options."; exit 2 ;;
  esac
}

if [[ -z "$MODE" ]]; then
  choose || { printf '\nNothing selected. Pass a flag instead:\n\n' >&2; sed -n '7,13p' "$0" >&2; exit 2; }
fi

# --- run --------------------------------------------------------------------
head2 "PolinRider cleaner"
say "${DIM}Everything below is read-only. Nothing is changed.${X}"
rule

if [[ $DO_ALL -eq 1 ]]; then
  warn "Order matters: the machine first, then credentials, then GitHub."
  say  "${DIM}Cleaning the remote while an infected machine still holds a valid token${X}"
  say  "${DIM}puts you back where you started within minutes.${X}"
fi

case "$MODE" in
  machine) scan_machine ;;
  path)    scan_path ;;
  org)     [[ -n "$ORG" ]] && scan_github org  "$ORG" ;;
  user)    [[ -n "$USR" ]] && scan_github user "$USR" ;;
esac

if [[ $DO_ALL -eq 1 ]]; then
  rule
  if [[ $WORST -ge 2 ]]; then
    warn "This machine has a confirmed finding."
    say  "Rotate your credentials from a different machine before scanning GitHub,"
    say  "or the account you are about to clean is still in someone else's hands."
    if ! ask "Continue to the GitHub scan anyway?" "n"; then
      say "Stopping here. Re-run with --org or --user once the machine is dealt with."
      exit $WORST
    fi
  fi
  [[ -n "$ORG" ]] && scan_github org  "$ORG"
  [[ -n "$USR" ]] && scan_github user "$USR"
fi

# --- what next --------------------------------------------------------------
rule
head2 "Where that leaves you"
case "$WORST" in
  0) good "Nothing confirmed against the current indicator set."
     say  ""
     say  "That is not the same as proven clean: signatures for this campaign rotate."
     say  "Rotate your GitHub tokens and SSH keys anyway, and scan again next week."
     ;;
  1) warn "Nothing confirmed, but some items need a human look."
     say  ""
     say  "Read each [review] line in the output above. Most are benign - the four"
     say  "usual ones are listed under \"False positives you will see\" in the README."
     ;;
  2) bad "Something confirmed. Do these in order:"
     say  ""
     say  "  1. Disconnect this machine from the network if the finding was local."
     say  "  2. Rotate every credential, from a different machine."
     say  "     ${DIM}README section: Step 2 - Rotate every credential${X}"
     say  "  3. Restore the affected branches."
     say  "     ${DIM}github-org-recovery/README.md, or github-account-recovery/README.md${X}"
     say  "  4. Quarantine the local artifacts once the machine is offline:"
     say  "     ${DIM}$LOCAL_TOOL --apply${X}"
     ;;
esac
say ""
say "${DIM}Full guide: README.md    Handing this to an AI agent: AGENTS.md${X}"
exit "$WORST"
