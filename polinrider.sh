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
#   --known-actor LOGIN
#                   someone you recognise among the accounts that pushed. It does
#                   NOT discount their pushes: this campaign force-pushes as
#                   whoever is logged in, so a familiar name is expected. It adds
#                   them to the list of machines that need checking. Repeatable.
#   --yes           never prompt. For scripts and AI agents
#   -h, --help      this text
#
# Exit codes: 0 nothing found, 1 review items only, 2 something confirmed,
#             3 a scan could not run. 3 says nothing about whether you are infected.

set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/common.sh
. "$HERE/lib/common.sh"
# The engines print standalone guidance by default. Driven from here, that
# guidance duplicates the menu, so it is suppressed rather than deleted: someone
# running lib/gh-scan.sh directly still needs it.
PRC_EMBEDDED=1; export PRC_EMBEDDED

MODE=""; ORG=""; USR=""; SCANPATH=""; OUT=""; ROOTS=""; ASSUME_YES=0; DO_ALL=0; PURGE=0
declare -a ROOT_LIST=()
TRUSTED_ARGS=()     # colleagues to name, so their machines get checked too

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
    --known-actor|--trusted-actor) TRUSTED_ARGS+=(--known-actor "$2"); shift 2 ;;
    -h|--help) sed -n '2,27p' "$0"; exit 0 ;;
    *) printf 'unknown argument: %s\nTry --help\n' "$1" >&2; exit 2 ;;
  esac
done

# Evidence never lands in the working directory. See prc_assert_safe_out.
OUT="${OUT:-$(prc_default_evidence_dir)}"

# --- presentation -----------------------------------------------------------
# All drawing lives in ui/. Nothing in there reads a repository or decides what
# is infected, so the presentation layer can be reviewed on its own and the
# scanning logic stays readable without it.
PRC_VERSION="$(sed -n 's/^## \[\?\(v[0-9.]*\).*/\1/p' "$HERE/CHANGELOG.md" 2>/dev/null | head -1)"
PRC_VERSION="${PRC_VERSION:-$(git -C "$HERE" describe --tags --abbrev=0 2>/dev/null || true)}"
# shellcheck source=ui/render.sh
. "$HERE/ui/render.sh"

# The old names, kept so the engines and the self-tests do not all have to
# change at once. Each is now one line of delegation to the UI layer.
B="$C_BOLD"; DIM="$C_DIM"; X="$C_RESET"
say()  { printf '%s\n' "$*"; }
head2(){ ui_section "$*"; }
rule() { ui_rule; }
step() { ui_step "$*"; }
warn() { ui_warn "$*"; }
bad()  { ui_bad "$*"; }
good() { ui_ok "$*"; }

ask() {  # ask <prompt> <default y|n>  -> 0 for yes
  [[ $ASSUME_YES -eq 1 ]] && return 0
  ui_ask "$1" "${2:-n}"
}

# --- what machine is this ---------------------------------------------------
OSNAME="$(uname -s 2>/dev/null || echo unknown)"
case "$OSNAME" in
  Darwin)                      OS="macOS";   LOCAL_TOOL="$HERE/machine-cleanup/check-macos.sh" ;;
  Linux)                       OS="Linux";   LOCAL_TOOL="$HERE/machine-cleanup/check-linux.sh" ;;
  MINGW*|MSYS*|CYGWIN*|Windows_NT) OS="Windows"; LOCAL_TOOL="" ;;
  *)                           OS="$OSNAME"; LOCAL_TOOL="$HERE/machine-cleanup/check-linux.sh" ;;
esac

# Directories worth offering. Detected, never assumed: the list is shown and
# confirmed before anything is scanned, because where people keep code varies
# far too much to guess.
default_roots() {
  local d
  for d in "$HOME/Sites" "$HOME/Projects" "$HOME/code" "$HOME/dev" "$HOME/src" \
           "$HOME/work" "$HOME/git" "$HOME/projects" "$HOME/repos" "$HOME/Developer" \
           "$HOME/Documents" "$HOME/go/src"; do
    [[ -d "$d" ]] && printf '%s\n' "$d"
  done
}

# Places that hold code which runs on its own: a plugin loaded at session start,
# an extension loaded when the editor opens. Easy to forget and more interesting
# to an attacker than a documents folder.
runner_roots() {
  local d
  for d in "$HOME/.claude/plugins" "$HOME/.vscode/extensions" "$HOME/.vscode-insiders/extensions" \
           "$HOME/.cursor/extensions" "$HOME/.config/Code/User" \
           "$HOME/Library/Application Support/Code/User"; do
    [[ -d "$d" ]] && printf '%s\n' "$d"
  done
}

# ask_owner <org|user> - prompt until the name resolves on GitHub, or give up
# after three tries. A typo should cost a retry, not the whole session.
ask_owner() {  # ask_owner <org|user> [allow-blank]
  local kind="$1" blank_ok="${2:-0}" label probe name tries=0
  if [[ "$kind" == "org" ]]; then label="Organization name:"; probe="orgs"
  else label="Your GitHub username:"; probe="users"; fi
  while :; do
    name="$(ui_prompt "$label" "$([[ "$blank_ok" == 1 ]] && printf '[blank to skip]')")" || return 1
    if [[ -z "$name" ]]; then
      [[ "$blank_ok" == 1 ]] && return 0        # skipping is a real answer here
      continue                                   # a stray Enter is not
    fi
    if ! have gh; then printf '%s\n' "$name"; return 0; fi   # cannot check, take it
    if gh api "$probe/$name" --jq .login >/dev/null 2>&1; then
      printf '%s\n' "$name"; return 0
    fi
    tries=$((tries+1))
    {
      if gh api "users/$name" --jq .type 2>/dev/null | grep -q .; then
        ui_bad "$name exists but is not $([[ "$kind" == org ]] && echo "an organization" || echo "a user account")."
        ui_dim "try the other option from the first menu"
      elif gh auth status >/dev/null 2>&1; then
        ui_bad "GitHub does not know $probe/$name."
        ui_dim "check the spelling, or that your account can see it if it is private"
      else
        ui_bad "cannot check $name: gh is not signed in."
        ui_dim "run: gh auth login"
        printf '%s\n' "$name"; return 0
      fi
      [[ $tries -ge 3 ]] && { ui_warn "Three tries, stopping."; return 1; }
    } >&2
  done
}

# ask_roots - confirm what to scan. Fills the ROOT_LIST array.
ask_roots() {
  local pick typed d
  local -a found=() runners=()
  while IFS= read -r d; do [[ -n "$d" ]] && found+=("$d"); done < <(default_roots)
  while IFS= read -r d; do [[ -n "$d" ]] && runners+=("$d"); done < <(runner_roots)
  {
    ui_blank
    if [[ ${#found[@]} -gt 0 ]]; then
      ui_text "Code directories found under your home:"
      for d in "${found[@]}"; do ui_bullet "${d/#$HOME/~}"; done
    fi
    if [[ ${#runners[@]} -gt 0 ]]; then
      ui_blank; ui_text "Also worth checking, code that runs by itself:"
      for d in "${runners[@]}"; do ui_bullet "${d/#$HOME/~}"; done
    fi
  } >&2
  pick="$(ui_menu "Where should I look?" \
    "The directories found above" \
    "Those, plus the ones that run by themselves" \
    "Let me type the list myself" \
    "My whole home directory      slower, but misses nothing under \$HOME")" || return 1
  ROOT_LIST=()
  case "$pick" in
    1) ROOT_LIST=(${found[@]+"${found[@]}"}) ;;
    2) ROOT_LIST=(${found[@]+"${found[@]}"} ${runners[@]+"${runners[@]}"}) ;;
    3) typed="$(ui_prompt "Directories, space separated:")" || return 1
       # shellcheck disable=SC2206  # splitting on whitespace is the documented shape
       ROOT_LIST=(${typed//\~/$HOME}) ;;
    4) ROOT_LIST=("$HOME") ;;
  esac
  [[ ${#ROOT_LIST[@]} -gt 0 ]]
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
ERRORED=0          # a scan that could not run. Never a statement about the code.
ERRORS=""
FOUND_IN=""        # machine | github | path, for the advice at the end
GH_KIND=""; GH_NAME=""
# Exit code 3 means "could not scan". It must never reach WORST, or a missing
# dependency and a bad path get reported as a confirmed infection, which is
# what used to happen: `--path /nowhere` printed the full compromise playbook.
note_rc() {
  if [[ "$1" -ge 3 ]]; then ERRORED=1; return 0; fi
  [[ "$1" -gt "$WORST" ]] && WORST="$1"
  return 0
}
note_error() { ERRORED=1; ERRORS="${ERRORS}${1}"$'\n'; }

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
  check_deps 0 || { note_error "the machine scan needs git"; return 3; }
  local d
  ROOT_LIST=()
  if [[ -n "$ROOTS" ]]; then
    # shellcheck disable=SC2206  # --roots is documented as space separated
    ROOT_LIST=($ROOTS)
  elif [[ $ASSUME_YES -eq 1 ]]; then
    while IFS= read -r d; do [[ -n "$d" ]] && ROOT_LIST+=("$d"); done < <(default_roots)
  else
    ask_roots || return 1
  fi
  if [[ ${#ROOT_LIST[@]} -eq 0 ]]; then
    warn "No common code directory found under \$HOME."
    say  "Re-run with the real ones, for example:  --roots \"\$HOME/work \$HOME/src\""
    return 1
  fi
  for d in "${ROOT_LIST[@]}"; do ui_bullet "${d/#$HOME/~}"; done
  say "${DIM}this reads only; the first run takes a few minutes${X}"
  "$LOCAL_TOOL" "${ROOT_LIST[@]}" 2>&1 | ui_findings
  local rc="${PIPESTATUS[0]}"; [[ $rc -eq 2 ]] && FOUND_IN="machine"; note_rc $rc; return $rc
}

scan_path() {
  step "Scanning $SCANPATH"
  if [[ ! -d "$SCANPATH" ]]; then
    bad "no such directory: $SCANPATH"
    note_error "cannot scan $SCANPATH: no such directory"
    return 3
  fi
  check_deps 0 || { note_error "scanning a folder needs git"; return 3; }
  local extra=""
  [[ -d "$SCANPATH/.git" ]] && extra="--all-refs"
  # shellcheck disable=SC2086  # extra is one optional flag
  "$HERE/ci/scan-workspace.sh" --path "$SCANPATH" $extra 2>&1 | ui_findings
  local rc="${PIPESTATUS[0]}"; [[ $rc -eq 2 ]] && FOUND_IN="path"; note_rc $rc; return $rc
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
  check_deps 1 || { note_error "the GitHub scan needs git, gh and jq, and gh must be signed in"; return 3; }
  OUT="$(prc_prepare_out "$OUT")" || { note_error "cannot prepare the evidence directory"; return 3; }
  ui_dim "mirror-cloning every repository, this can take a while"
  legacy_evidence_hint
  if [[ "$kind" == "org" ]]; then
    "$dir/scan.sh" --org "$name" --out "$OUT" 2>&1 | ui_stream
    [[ "${PIPESTATUS[0]}" -eq 0 ]] || { note_error "the scan of $name did not finish"; return 3; }
  else
    "$dir/scan.sh" --user "$name" --out "$OUT" 2>&1 | ui_stream
    [[ "${PIPESTATUS[0]}" -eq 0 ]] || { note_error "the scan of $name did not finish"; return 3; }
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
  local pick
  pick="$(ui_menu "What do you want to check?" \
    "This computer            files, extensions, persistence, an installed implant" \
    "A GitHub organization    every repository and branch" \
    "My GitHub account        every repository you own" \
    "One folder or repo       the quickest check" \
    "All of it, in order      if you think you were hit")" || return 1
  case "$pick" in
    1) MODE="machine" ;;
    2) MODE="org";  ORG="$(ask_owner org)"  || return 1 ;;
    3) MODE="user"; USR="$(ask_owner user)" || return 1 ;;
    4) MODE="path"
       local tries=0
       while :; do
         SCANPATH="$(ui_prompt "Path to the folder:")" || return 1
         SCANPATH="${SCANPATH/#\~/$HOME}"        # the shell does not expand ~ from read
         [[ -d "$SCANPATH" ]] && break
         ui_bad "no such directory: $SCANPATH"
         if [[ "$OS" == "macOS" && "$SCANPATH" == /home/* ]]; then
           local guess="/Users/${SCANPATH#/home/}"
           [[ -d "${guess%/}" ]] && ui_dim "on macOS that is probably ${guess%/}"
         fi
         tries=$((tries+1)); [[ $tries -ge 3 ]] && return 1
       done ;;
    5) DO_ALL=1; MODE="machine"
       ORG="$(ask_owner org 1 || true)"
       if [[ -z "$ORG" ]]; then USR="$(ask_owner user 1 || true)"; fi ;;
  esac
}

# --purge-evidence is not a scan, so it must not fall into the mode menu.
ui_banner
ui_blank
# The facts are gathered here, not in ui/, so the presentation layer keeps
# running no external commands and stays skippable in an audit.
_gh_who=""
if have gh; then
  _gh_who="$(gh api user --jq .login 2>/dev/null || true)"
  if [[ -n "$_gh_who" ]]; then _gh_who="gh signed in as $_gh_who"; else _gh_who="gh installed, not signed in"; fi
else _gh_who="gh not installed"; fi
ui_status "$OS  $S_DOT  $_gh_who"
# A /var/folders path is unreadable and tells nobody anything. Name the variable.
_ev="$OUT"; _note=""
if [[ -n "${TMPDIR:-}" && "$_ev" == "${TMPDIR%/}"/* ]]; then
  _ev="\$TMPDIR/${_ev#"${TMPDIR%/}"/}"; _note="  $S_DOT  cleared on restart"
else
  _ev="${_ev/#$HOME/~}"
fi
ui_status "Evidence $S_ARROW $_ev$_note"
ui_blank
if [[ -z "$MODE" && $PURGE -eq 0 ]]; then
  choose || {
    ui_blank
    ui_dim "Nothing selected. You can also say what to scan directly:"
    ui_blank
    sed -n '9,15p' "$0" | sed 's/^#   //;s/^#//' | while read -r l; do
      [[ -n "$l" ]] && printf '    %s\n' "$l"
    done
    ui_blank
    exit 2
  }
fi

# --- run --------------------------------------------------------------------
# The banner already said what this is. Repeating it here, ruled twice, just
# pushed the first finding further down the screen.
ui_blank
ui_dim "Read-only. Nothing is changed. No warranty, no liability: DISCLAIMER.md"

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


# --- guided cleanup ---------------------------------------------------------
# The point of this: nobody should have to read a manual, assemble flags, and
# guess which of two remedies applies. One question at a time, a dry run before
# anything is written, and an explicit yes before it happens.
pick_repo() {  # pick_repo <repos-file> -> echoes the chosen repo
  local file="$1" n i=1 reply
  n=$(awk 'END{print NR}' "$file")
  printf '\n  %sWhich repository?%s\n\n' "$C_BOLD" "$C_RESET" >&2
  while read -r r; do printf '    %s%2s%s  %s\n' "$C_CYAN" "$i" "$C_RESET" "$r" >&2; i=$((i+1)); done < "$file"
  while :; do
    printf '\n  %s1-%s, or q to go back:%s ' "$C_BOLD" "$n" "$C_RESET" >&2
    read -r reply || return 1
    case "$reply" in
      [Qq]*)    return 1 ;;
      "")       continue ;;
      *[!0-9]*) printf '  %snot a number. Pick 1 to %s, or q.%s\n' "$C_DIM" "$n" "$C_RESET" >&2; continue ;;
    esac
    if [[ "$reply" -ge 1 && "$reply" -le "$n" ]]; then
      awk -v k="$reply" 'NR==k{print; exit}' "$file"; return 0
    fi
    printf '  %sthere is no repository %s. Pick 1 to %s, or q.%s\n' "$C_DIM" "$reply" "$n" "$C_RESET" >&2
  done
}

# ask_mode - how thorough. Returns "additive" or "rewrite" on stdout.
ask_mode() {
  local pick
  {
    ui_blank
    ui_text "There are two ways to get the payload out, and they are not equivalent."
    ui_blank
    ui_text "Remove it   Adds one commit per branch deleting the files. Nothing is"
    ui_text "            rewritten, so you can revert it. The payload stays in the"
    ui_text "            history: an old commit can still be checked out and run."
    ui_blank
    ui_text "Erase it    Takes the files out of every commit and force-pushes every"
    ui_text "            ref. Every commit id changes, existing clones diverge, and"
    ui_text "            the old commits stay fetchable from GitHub by SHA until"
    ui_text "            Support runs gc. Nothing left to check out."
  } >&2
  pick="$(ui_menu "How thorough?" \
    "Remove it    safe, reversible, history keeps the payload" \
    "Erase it     thorough, rewrites history, force-pushes everything")" || return 1
  case "$pick" in 1) printf 'additive\n' ;; 2) printf 'rewrite\n' ;; esac
}

clean_one() {  # clean_one <recovery-dir> <out> <repo> <mode> [n] [total]
  local dir="$1" out="$2" repo="$3" mode="$4" n="${5:-}" total="${6:-}" extra=""
  [[ "$mode" == "rewrite" ]] && extra="--rewrite"
  if [[ -n "$n" && -n "$total" ]]; then
    ui_step "[$n/$total] $repo"
  else
    ui_step "$repo"
  fi
  ui_dim "dry run, $mode. Nothing is written; this only shows what would change."
  ui_dim "The first step clones the repository, which is the slow part."
  # shellcheck disable=SC2086  # extra is one optional flag
  ui_dim "cloning and inspecting; git prints its own progress below"
  "$HERE/$dir/clean-repo.sh" "$repo" $extra --out "$out" 2>&1
  if [[ "$mode" == "rewrite" ]]; then
    ui_blank
    ui_warn "Erasing touches every branch and tag in the repository, not only the"
    ui_warn "ones listed above. That is deliberate: a payload left on any ref can"
    ui_warn "be checked out."
  fi
  if ask "Apply that to $repo now?" "n"; then
    ui_step "Applying to $repo"
    # shellcheck disable=SC2086
    if "$HERE/$dir/clean-repo.sh" "$repo" $extra --apply --out "$out" 2>&1; then
      ui_ok "$repo done"
    else
      ui_warn "$repo finished with errors. Read the lines above."
    fi
  else
    ui_dim "left alone"
  fi
}

guided_cleanup() {  # guided_cleanup <recovery-dir> <out>
  local dir="$1" out="$2" repos="$2/affected-repos.txt" choice mode repo
  local restorable="$2/restorable-repos.txt" targets="$2/restore-targets.tsv"
  [[ -s "$repos" ]] || return 0
  [[ $ASSUME_YES -eq 1 ]] && return 0    # a script asked for no prompts

  local can_restore=0 n_restore=0
  if [[ -s "$restorable" ]]; then can_restore=1; n_restore="$(awk 'END{print NR}' "$restorable")"; fi

  while :; do
    local -a opts=("Read what was actually found")
    local -a acts=("read")
    if [[ "$can_restore" -eq 1 ]]; then
      if [[ -s "$targets" ]]; then
        opts+=("Restore branches                 $n_restore repo(s), moves refs back, no files edited")
        acts+=("restore")
      else
        opts+=("Fetch the pre-attack commits     needed before restoring, $n_restore repo(s)")
        acts+=("preserve")
      fi
    else
      # Shown, greyed, with the reason: a missing option and an impossible one
      # look identical otherwise.
      opts+=("!Restore branches                 unavailable for these repositories" \
             "~GitHub keeps about 300 push events per repository for about 90 days." \
             "~None survive here, so there is no recorded earlier state to move a" \
             "~branch back to. A limit of the GitHub API, not of this tool.")
    fi
    opts+=("Clean one repository, dry run first" \
           "Clean every affected repository, one at a time" \
           "Stop here, the plan is written to NEXT-STEPS.md")
    acts+=("one" "all" "stop")

    choice="$(ui_menu "What do you want to do?" "${opts[@]}")" || return 0
    case "${acts[$((choice-1))]}" in
      read) ui_step "What matched"
            if [[ -f "$out/triage.txt" ]]; then
              ui_dim "full file: $out/triage.txt"; ui_pager "$out/triage.txt"
            else ui_warn "no triage.txt in $out"; fi ;;
      preserve)
            ui_step "Fetching the pre-attack commits"
            ui_dim "reads from GitHub, pushes nothing. Each one is checked for the payload."
            "$HERE/$dir/preserve-restore-points.sh" --out "$out" 2>&1 ;;
      restore)
            ui_step "Restoring is the better fix, and it is not automated here"
            ui_text "Moving a branch back is the one genuinely destructive thing in this"
            ui_text "repository, so it stays behind its own preflight and a person who has"
            ui_text "read the plan. The verified targets are in:"
            ui_file_line "targets" "$targets"
            ui_file_line "plan" "$out/NEXT-STEPS.md"
            ui_blank
            ui_text "The commands, with your values filled in, are in the plan under"
            ui_text "\"How it got there\"." ;;
      one)  repo="$(pick_repo "$repos")" || continue
            mode="$(ask_mode)" || continue
            clean_one "$dir" "$out" "$repo" "$mode" ;;
      all)  mode="$(ask_mode)" || continue
            ui_step "Every affected repository"
            ui_dim "$(awk 'END{print NR}' "$repos") repositories, $mode, each with its own dry run"
            local n=0 total; total="$(awk 'END{print NR}' "$repos")"
            while read -r repo; do
              [[ -z "$repo" ]] && continue
              n=$((n+1))
              clean_one "$dir" "$out" "$repo" "$mode" "$n" "$total"
            done < "$repos" ;;
      stop) return 0 ;;
    esac
  done
}

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
      --owner "$GH_NAME" --owner-type "$GH_KIND" \
      ${TRUSTED_ARGS[@]+"${TRUSTED_ARGS[@]}"} || {
    say "  Could not generate the plan. Read $OUT/triage.txt by hand."
    return 0
  }

  ui_blank
  ui_text "Everything, in order, with each command written out:"
  ui_file_line "plan" "$OUT/NEXT-STEPS.md"
  [[ -f "$OUT/triage.txt" ]] && ui_file_line "what matched" "$OUT/triage.txt"

  guided_cleanup "$dir" "$OUT"
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

# An error is reported before any verdict, and it cancels the clean result.
# "Nothing found" and "could not look" are different answers.
if [[ $ERRORED -eq 1 ]]; then
  bad "A check could not run. This is not a clean result."
  say ""
  printf '%s' "$ERRORS" | grep -v '^$' | sed 's/^/  /'
  say ""
  if [[ "$WORST" -eq 0 ]]; then
    say "  Nothing was found by the checks that did run, but at least one did not"
    say "  run at all, so nothing here says you are clean. Fix the above and"
    say "  re-run before you conclude anything."
    say ""
    say "${DIM}Full guide: README.md    Handing this to an AI agent: AGENTS.md${X}"
    exit 3
  fi
  say "  The findings below come only from the checks that did complete."
  say ""
fi

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
# Confirmed beats incomplete: a caller that acts on 2 should still see it.
if [[ "$WORST" -lt 2 && $ERRORED -eq 1 ]]; then exit 3; fi
exit "$WORST"
