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
#   --out DIR       where evidence goes. Default: a directory under $TMPDIR,
#                   which your machine clears on reboot. Mirrors hold live
#                   malware, so they are never written inside a git checkout
#                   and are not meant to outlive the incident.
#   --purge-evidence  delete the evidence directory and everything in it
#   --yes           never prompt. For scripts and AI agents
#   -h, --help      this text
#
# Exit codes: 0 nothing found, 1 review items only, 2 something confirmed.

set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/common.sh
. "$HERE/lib/common.sh"

MODE=""; ORG=""; USR=""; SCANPATH=""; OUT=""; ROOTS=""; ASSUME_YES=0; DO_ALL=0; PURGE=0

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
    --purge-evidence) PURGE=1; shift ;;
    -h|--help) sed -n '2,27p' "$0"; exit 0 ;;
    *) printf 'unknown argument: %s\nTry --help\n' "$1" >&2; exit 2 ;;
  esac
done

# Evidence never lands in the working directory. See prc_assert_safe_out.
OUT="${OUT:-$(prc_default_evidence_dir)}"

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
FOUND_IN=""        # machine | github | path, for the advice at the end
GH_KIND=""; GH_NAME=""
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
  local rc=$?; [[ $rc -ge 2 ]] && FOUND_IN="machine"; note_rc $rc; return $rc
}

scan_path() {
  step "Scanning $SCANPATH"
  check_deps 0 || return 2
  local extra=""
  [[ -d "$SCANPATH/.git" ]] && extra="--all-refs"
  # shellcheck disable=SC2086  # extra is one optional flag
  "$HERE/ci/scan-workspace.sh" --path "$SCANPATH" $extra
  local rc=$?; [[ $rc -ge 2 ]] && FOUND_IN="path"; note_rc $rc; return $rc
}

# Earlier versions wrote mirrors to ./evidence, inside the checkout. If that is
# still there, say so once and offer the move, rather than silently re-cloning
# every repository into the new location.
legacy_evidence_hint() {
  local legacy n
  for legacy in "$PWD/evidence" "$HOME/.polinrider/evidence"; do
    [[ -d "$legacy" && "$legacy" != "$OUT" ]] || continue
    n=$(find "$legacy" -maxdepth 1 -name '*.git' 2>/dev/null | grep -c . || true)
    [[ "${n:-0}" -gt 0 ]] || continue
    say ""
    warn "$n mirror(s) are still in $legacy"
    say "  Older versions put them there. They are infected repositories sitting"
    say "  in a directory nothing clears. Move them here and this run reuses them"
    say "  instead of cloning everything again:"
    say ""
    say "    ${DIM}mv $legacy/* $OUT/ && rmdir $legacy${X}"
    say ""
  done
}

scan_github() {  # scan_github <org|user> <name>
  local kind="$1" name="$2" dir
  if [[ "$kind" == "org" ]]; then dir="$HERE/github-org-recovery"; else dir="$HERE/github-account-recovery"; fi
  step "Scanning the $([[ "$kind" == org ]] && echo "organization" || echo "account") $name"
  check_deps 1 || return 2
  OUT="$(prc_prepare_out "$OUT")" || return 2
  say "${DIM}mirror-cloning every repository into${X}"
  say "  ${B}$OUT${X}"
  say "${DIM}this can take a while. Evidence is kept outside your working directory${X}"
  say "${DIM}on purpose: the mirrors hold live malware.${X}"
  legacy_evidence_hint
  if [[ "$kind" == "org" ]]; then
    "$dir/scan.sh" --org "$name" --out "$OUT" || return 2
  else
    "$dir/scan.sh" --user "$name" --out "$OUT" || return 2
  fi
  step "Separating real findings from your own detection tooling"
  # triage-filter exits 0 by design, so its exit code must never be the verdict.
  # An earlier version returned it directly, which reported "nothing confirmed"
  # on an account with 312 infected refs. The verdict comes from the counts.
  local filtered real reviewed
  filtered="$("$dir/triage-filter.sh" "$OUT/triage.json" 2>&1)"
  printf '%s\n' "$filtered"
  real="$(printf '%s\n' "$filtered" | awk -F: '/REAL_SUSPECT refs/ {gsub(/[^0-9]/,"",$2); print $2; exit}')"
  reviewed="$(jq '[.[]|select(.verdict=="review")]|length' "$OUT/triage.json" 2>/dev/null || echo 0)"
  [[ "$real"     =~ ^[0-9]+$ ]] || real=0
  [[ "$reviewed" =~ ^[0-9]+$ ]] || reviewed=0
  if [[ "$real" -gt 0 ]]; then
    bad "$real ref(s) carry a confirmed indicator and are not your own detection tooling."
    FOUND_IN="github"; GH_KIND="$kind"; GH_NAME="$name"; REAL_REFS="$real"
    note_rc 2; return 2
  fi
  if [[ "$reviewed" -gt 0 ]]; then note_rc 1; return 1; fi
  note_rc 0; return 0
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

# --purge-evidence is not a scan, so it must not fall into the mode menu.
if [[ -z "$MODE" && $PURGE -eq 0 ]]; then
  choose || { printf '\nNothing selected. Pass a flag instead:\n\n' >&2; sed -n '7,13p' "$0" >&2; exit 2; }
fi

# --- run --------------------------------------------------------------------
head2 "PolinRider cleaner"
say "${DIM}Everything below is read-only. Nothing is changed.${X}"
say "${DIM}Independent open source tool. No warranty, no liability. See DISCLAIMER.md.${X}"

if [[ $PURGE -eq 1 ]]; then
  if [[ ! -d "$OUT" ]]; then
    good "Nothing to delete. $OUT does not exist."
    exit 0
  fi
  n=$(find "$OUT" -maxdepth 1 -name '*.git' 2>/dev/null | grep -c . || true)
  say ""
  warn "About to delete $OUT"
  say "  ${DIM}$n mirror(s), $(du -sh "$OUT" 2>/dev/null | cut -f1) on disk${X}"
  say "  This includes triage.json, NEXT-STEPS.md and the push ledger. If a"
  say "  restore is still outstanding, the pre-attack commits go with them."
  if ask "Delete it?" "n"; then
    rm -rf "$OUT" && good "Deleted $OUT"
  else
    say "Left alone."
  fi
  exit 0
fi
prc_evidence_warn_stale "$OUT"
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

# --- what to actually do -----------------------------------------------------
github_playbook() {
  local dir
  if [[ "$GH_KIND" == "org" ]]; then
    dir="github-org-recovery"
  else
    dir="github-account-recovery"
  fi
  say "  ${B}$REAL_REFS ref(s) carry a real indicator.${X}"
  say ""
  say "  ${B}Do not open an affected repository in your editor.${X}"
  say "  The payload includes a .vscode/tasks.json with \"runOn\": \"folderOpen\","
  say "  which runs as soon as VS Code opens the folder. Do not git pull in an"
  say "  existing clone either: pulling an infected clone re-infects the remote."
  say ""

  # Everything below is generated from the scan, with real repository names,
  # real paths and a real timestamp. An earlier version printed <T0> and
  # <two hours before the first bad push> as literal text, which is a command
  # that fails the moment you paste it.
  "$HERE/lib/next-steps.sh" --triage "$OUT/triage.json" --out "$OUT" \
      --owner "$GH_NAME" --owner-type "$GH_KIND" || {
    say "  Could not generate the plan. Read $OUT/triage.txt by hand."
    return 0
  }

  say ""
  say "  ${B}Everything, in order, with each command written out:${X}"
  say "    ${DIM}$OUT/NEXT-STEPS.md${X}"
  say ""
  say "  Background: ${DIM}$dir/README.md${X}"
}

path_playbook() {
  say "  ${B}This folder contains a confirmed indicator.${X} In order:"
  say ""
  say "  ${B}1. Do not open it in your editor${X} until it is dealt with. The usual"
  say "     trigger is a .vscode/tasks.json that runs on folder open."
  say ""
  say "  ${B}2. Read the finding lines above.${X} They name the file and what matched."
  say "     A fake font under public/fonts and a payload appended to a build config"
  say "     are the two common shapes."
  say ""
  say "  ${B}3. Check the machine${X}, because whatever put it there had access:"
  say "     ${DIM}./polinrider.sh --machine${X}"
  say "     If the payload ran here, assume it read what this account could reach."
  say "     Rotate every credential once you know which machines are clean."
  say "     ${DIM}README, Step 2. Rotate every credential${X}"
  say ""
  say "  ${B}4. Check the remote.${X} If this folder is a clone, the same payload is"
  say "     almost certainly pushed:"
  say "     ${DIM}./polinrider.sh --user YOUR-USERNAME${X}"
  say ""
  say "  ${B}5. Do not fix it in this clone and push.${X} Clean the remote first, then"
  say "     delete this clone and clone again. A push from an infected clone puts"
  say "     the payload straight back."
}

machine_playbook() {
  say "  ${B}This machine has a confirmed indicator.${X} In order:"
  say ""
  say "  ${B}1. Disconnect it from the network.${X}"
  say ""
  say "  ${B}2. Rotate every credential, from a different machine.${X}"
  say "     ${DIM}README, Step 2. Rotate every credential${X}"
  say "     If a crypto wallet or seed phrase was on this machine, move the funds."
  say ""
  say "  ${B}3. Read the report before changing anything.${X}"
  say "     It lists what matched and where."
  say ""
  say "  ${B}4. Quarantine the artifacts.${X} Nothing is deleted; everything is moved"
  say "     to a timestamped folder with a manifest."
  say "     ${DIM}$LOCAL_TOOL --apply${X}"
  say "     If a persistence entry was found, stop it first with the command the"
  say "     report prints. Moving the file does not stop what it already started."
  say ""
  say "  ${B}5. Decide whether to rebuild.${X} If any persistence artifact was found,"
  say "     rebuild from a clean install. Do not restore a backup from after the"
  say "     infection date."
  say "     ${DIM}README, \"Should the machine be rebuilt?\"${X}"
  say ""
  say "  ${B}6. Then check your GitHub repositories${X}, because the credentials on"
  say "     this machine were reachable."
  say "     ${DIM}./polinrider.sh --user YOUR-USERNAME${X}"
}

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
     say  "Read the [review] lines above. The [info] lines are inventory and"
     say  "hardening advice, not findings, and need nothing from you."
     say  "The usual benign matches are listed under \"False positives you will"
     say  "see\" in the README."
     ;;
  2) bad "Something confirmed."
     say  ""
     case "$FOUND_IN" in
       github) github_playbook ;;
       machine) machine_playbook ;;
       path)   path_playbook ;;
       *)      say  "  Read what matched, then follow the recovery steps in the README."
               say  "     ${DIM}README, Step 2 onwards${X}" ;;
     esac
     ;;
esac
say ""
say "${DIM}Full guide: README.md    Handing this to an AI agent: AGENTS.md${X}"
if [[ "$WORST" -ge 2 ]]; then
  say ""
  rule
  say "${DIM}Before you change anything: this tool is provided as is, with no warranty${X}"
  say "${DIM}and no liability. Restoring branches and quarantining files are your${X}"
  say "${DIM}decisions, taken with your credentials and your authorisation. If this is${X}"
  say "${DIM}an organization account, make sure you are the one who is allowed to do it.${X}"
  say "${DIM}See DISCLAIMER.md.${X}"
fi
exit "$WORST"
