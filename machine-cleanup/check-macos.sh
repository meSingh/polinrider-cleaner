#!/usr/bin/env bash
# check-macos.sh - PolinRider check and cleanup for macOS.
#
# Default is a dry run: it inspects and reports, and changes nothing.
# --apply moves confirmed artifacts into a quarantine directory. It never deletes.
#
# Usage:
#   ./check-macos.sh [--apply] [--background] [--resume [DIR]] [--jobs N]
#                    [--quarantine DIR] [--report FILE] [ROOT ...]
#
# ROOT is a directory holding your code. Defaults to ~/Sites ~/Projects ~/code
# ~/dev ~/Documents. Give the real ones, the scan is only as good as its roots.
#
# --background  run detached, under caffeinate so the machine stays awake. The
#               terminal can be closed. It does not survive a logout or reboot.
# --resume      pick up an interrupted run: reuse its file list, skip the checks
#               it finished, keep its counts. Newest run unless DIR is given.
#
# Exit codes: 0 clean, 1 review items only, 2 confirmed indicator hit,
#             3 could not scan.

set -uo pipefail
# shellcheck source=lib/local-common.sh
. "$(cd "$(dirname "$0")" && pwd)/../lib/local-common.sh"

TS="$(date -u +%Y%m%dT%H%M%SZ)"
# shellcheck disable=SC2034  # QDIR and REPORT are read by lib/local-common.sh
QDIR="$HOME/polinrider-quarantine-$TS"
REPORT="$HOME/polinrider-report-$TS.txt"
ROOTS=()
BACKGROUND=0; RESUME=0; RESUME_DIR=""; STATE_ARG=""
PASS=()      # the arguments handed to the detached copy, minus --background

while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply)      APPLY=1; PASS+=("$1"); shift ;;
    --background) BACKGROUND=1; shift ;;
    --resume)     RESUME=1; PASS+=("$1")
                  if [[ $# -gt 1 && -d "$2" ]]; then RESUME_DIR="$2"; PASS+=("$2"); shift; fi
                  shift ;;
    --state)      STATE_ARG="$2"; shift 2 ;;
    --jobs)       PRC_JOBS="$2"; PASS+=("$1" "$2"); shift 2 ;;
    --quarantine) QDIR="$2"; PASS+=("$1" "$2"); shift 2 ;;
    --report)     REPORT="$2"; PASS+=("$1" "$2"); shift 2 ;;
    -h|--help)    sed -n '2,21p' "$0"; exit 0 ;;
    -*)           echo "unknown argument: $1" >&2; exit 3 ;;
    *)            ROOTS+=("$1"); PASS+=("$1"); shift ;;
  esac
done

# State directory: the file list, the checkpoints, the counters and the log.
if [[ $RESUME -eq 1 ]]; then
  STATE="$(prc_resume_dir "$RESUME_DIR")"
  [[ -n "$STATE" && -d "$STATE" ]] || { echo "nothing to resume under $HOME/polinrider-scan-*" >&2; exit 3; }
  [[ -f "$STATE/report-path" ]] && REPORT="$(cat "$STATE/report-path")"
  if [[ ${#ROOTS[@]} -eq 0 && -f "$STATE/roots" ]]; then
    while IFS= read -r d; do [[ -n "$d" ]] && ROOTS+=("$d"); done < "$STATE/roots"
  fi
else
  STATE="${STATE_ARG:-$HOME/polinrider-scan-$TS}"
fi
[[ ${#ROOTS[@]} -eq 0 ]] && ROOTS=("$HOME/Sites" "$HOME/Projects" "$HOME/code" "$HOME/dev" "$HOME/Documents")
prc_state_init "$STATE"
printf '%s\n' "${ROOTS[@]}" > "$STATE/roots"

# Detach. nohup survives the terminal closing; caffeinate -i keeps the machine
# from sleeping while the scan runs. Neither survives a logout, hence --resume.
if [[ $BACKGROUND -eq 1 && -z "${PRC_BG:-}" ]]; then
  LOG="$STATE/scan.log"
  if command -v caffeinate >/dev/null 2>&1; then
    PRC_BG=1 nohup caffeinate -i "$0" --state "$STATE" ${PASS[@]+"${PASS[@]}"} >"$LOG" 2>&1 </dev/null &
  else
    PRC_BG=1 nohup "$0" --state "$STATE" ${PASS[@]+"${PASS[@]}"} >"$LOG" 2>&1 </dev/null &
  fi
  echo "running in the background, pid $!. The terminal can be closed; the machine will not sleep."
  echo "  follow:   tail -f '$LOG'"
  echo "  report:   $REPORT"
  echo "  if it stops (logout, reboot):  $0 --resume '$STATE'"
  exit 0
fi

trap 'say ""; say "interrupted. Resume where it stopped:  $0 --resume \"$STATE\""; exit 130' INT TERM

[[ $RESUME -eq 1 ]] || : > "$REPORT"
prc_local_load_iocs
quarantine_init

say "PolinRider local check - macOS - $(date -u +%Y-%m-%dT%H:%M:%SZ)$([[ $RESUME -eq 1 ]] && echo ' (resumed)')"
say "host: $(hostname)   user: $(whoami)"
say "roots: ${ROOTS[*]}"
say "state: $STATE"
say "mode: $([[ $APPLY -eq 1 ]] && echo 'APPLY - confirmed artifacts will be moved to quarantine' || echo 'dry run - nothing will be changed')"

prc_walk "${ROOTS[@]}"

check_persistence() {
  hdr "Persistence: LaunchAgents, LaunchDaemons, cron"
  local d f
  for d in "$HOME/Library/LaunchAgents" "/Library/LaunchAgents" "/Library/LaunchDaemons"; do
    [[ -d "$d" ]] || continue
    while read -r f; do
      [[ -z "$f" ]] && continue
      if has_strong "$f"; then
        bad "launch item contains an indicator: $f"
        say "           after quarantine, unload it: launchctl unload '$f'"
        quarantine "$f" "launch-item"
      elif grep -qaE '(curl|wget|node|osascript|base64|python).*(http|-e |eval)' "$f" 2>/dev/null; then
        warn "launch item runs a network or interpreter command: $f"
      fi
    done < <(find "$d" -maxdepth 1 -name '*.plist' 2>/dev/null)
    info "$(find "$d" -maxdepth 1 -name '*.plist' -mtime -90 2>/dev/null | grep -c .) launch items in $d changed in the last 90 days, none containing an indicator. Listed in the report."
    find "$d" -maxdepth 1 -name '*.plist' -mtime -90 2>/dev/null | sed 's|^|    |' >> "$REPORT"
    info "Recent changes only. Persistence installed more than 90 days ago is not listed; its contents are still checked against the indicators."
  done
  local cron; cron="$(crontab -l 2>/dev/null)"
  if [[ -n "$cron" ]]; then
    warn "user crontab is not empty, review every line:"
    printf '%s\n' "$cron" | sed 's|^|    |' | tee -a "$REPORT"
  else
    ok "user crontab is empty"
  fi
}

check_connections() {
  hdr "Live connections from node and Electron processes"
  if command -v lsof >/dev/null 2>&1; then
    check_network "$(lsof -nP -iTCP -sTCP:ESTABLISHED 2>/dev/null | grep -Ei '(node|Code Helper|Cursor|Electron)' | head -40)"
  else
    warn "lsof not available, skipped"
  fi
}

run_check "Second-stage implant"        check_implants   "${ROOTS[@]}"
run_check "IDE extensions"              check_extensions "$HOME/.vscode/extensions" "$HOME/.vscode-insiders/extensions" \
                                                         "$HOME/.cursor/extensions" "$HOME/.windsurf/extensions" \
                                                         "$HOME/.vscode-oss/extensions"
run_check "Workspace tasks"             check_tasks_json  "${ROOTS[@]}"
run_check "Build configs"               check_configs     "${ROOTS[@]}"
run_check "Font files"                  check_fonts       "${ROOTS[@]}"
run_check "Propagation artifact"        check_propagation "$HOME"
run_check "Known-bad packages"          check_packages    "${ROOTS[@]}"
run_check "Persistence"                 check_persistence
run_check "Shell startup files"         check_shell_rc "$HOME/.zshrc" "$HOME/.zprofile" "$HOME/.zshenv" \
                                                       "$HOME/.bashrc" "$HOME/.bash_profile" "$HOME/.profile"
run_check "Git configuration and hooks" check_git      "${ROOTS[@]}"
run_check "npm configuration"           check_npm
run_check "Resident interpreters"       check_processes
run_check "Live connections"            check_connections
run_check "Credential surface"          check_credentials "${ROOTS[@]}"
verdict
exit $?
