#!/usr/bin/env bash
# check-linux.sh - PolinRider check and cleanup for Linux.
#
# Default is a dry run: it inspects and reports, and changes nothing.
# --apply moves confirmed artifacts into a quarantine directory. It never deletes.
#
# Usage:
#   ./check-linux.sh [--apply] [--background] [--resume [DIR]] [--jobs N]
#                    [--quarantine DIR] [--report FILE] [ROOT ...]
#
# ROOT is a directory holding your code. Defaults to ~/src ~/code ~/dev
# ~/projects ~/work ~/git. Give the real ones, the scan is only as good as its roots.
#
# --fs-only     only the checks that read the filesystem being scanned. Skips
#               live processes, sockets, npm config, crontab and $HOME
#               persistence, which describe the machine you are running on
#               rather than the disk you pointed at. Use it for a backup
#               drive, an external disk or a mounted image.
# --background  run detached (setsid + nohup). The terminal can be closed. It does
#               not survive a logout or reboot; the system may still sleep.
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
    --fs-only)    FS_ONLY=1; PASS+=("$1"); shift ;;
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
[[ ${#ROOTS[@]} -eq 0 ]] && ROOTS=("$HOME/src" "$HOME/code" "$HOME/dev" "$HOME/projects" "$HOME/work" "$HOME/git")
prc_state_init "$STATE" "$RESUME"
printf '%s\n' "${ROOTS[@]}" > "$STATE/roots"

# Detach. setsid puts it in its own session so closing the terminal does not
# reach it; nohup covers the HUP either way. A logout still kills it, hence
# --resume. Linux has no caffeinate; systemd-inhibit is used when present.
if [[ $BACKGROUND -eq 1 && -z "${PRC_BG:-}" ]]; then
  LOG="$STATE/scan.log"
  RUN=("$0" --state "$STATE" ${PASS[@]+"${PASS[@]}"})
  command -v systemd-inhibit >/dev/null 2>&1 && RUN=(systemd-inhibit --what=idle:sleep --why="polinrider scan" "${RUN[@]}")
  command -v setsid >/dev/null 2>&1 && RUN=(setsid "${RUN[@]}")
  PRC_BG=1 nohup "${RUN[@]}" >"$LOG" 2>&1 </dev/null &
  echo "running in the background, pid $!. The terminal can be closed."
  echo "  follow:   tail -f '$LOG'"
  echo "  report:   $REPORT"
  echo "  if it stops (logout, reboot):  $0 --resume '$STATE'"
  exit 0
fi

trap 'say ""; say "interrupted. Resume where it stopped:  $0 --resume \"$STATE\""; exit 130' INT TERM

[[ $RESUME -eq 1 ]] || : > "$REPORT"
prc_local_load_iocs
quarantine_init

say "PolinRider local check - Linux - $(date -u +%Y-%m-%dT%H:%M:%SZ)$([[ $RESUME -eq 1 ]] && echo ' (resumed)')"
say "host: $(hostname)   user: $(whoami)"
say "roots: ${ROOTS[*]}"
say "state: $STATE"
say "mode: $([[ $APPLY -eq 1 ]] && echo 'APPLY - confirmed artifacts will be moved to quarantine' || echo 'dry run - nothing will be changed')"

prc_walk "${ROOTS[@]}"

check_persistence() {
hdr "Persistence: systemd units, autostart, cron"
local d f
for d in "$HOME/.config/systemd/user" "/etc/systemd/system" "/usr/lib/systemd/system"; do
  [[ -d "$d" ]] || continue
  while read -r f; do
    [[ -z "$f" ]] && continue
    if has_strong "$f"; then
      bad "systemd unit contains an indicator: $f"
      say "           after quarantine, disable it: systemctl --user disable --now '$(basename "$f")'"
      quarantine "$f" "systemd-unit"
    elif grep -qaE '^(ExecStart|ExecStartPre)=.*(curl|wget|node|base64|python).*(http|-e |eval)' "$f" 2>/dev/null; then
      warn "systemd unit runs a network or interpreter command: $f"
    fi
  done < <(find "$d" -maxdepth 1 -name '*.service' -o -maxdepth 1 -name '*.timer' 2>/dev/null)
  info "$(find "$d" -maxdepth 1 \( -name '*.service' -o -name '*.timer' \) -mtime -90 2>/dev/null | grep -c .) units in $d changed in the last 90 days, none containing an indicator. Listed in the report."
  find "$d" -maxdepth 1 \( -name '*.service' -o -name '*.timer' \) -mtime -90 2>/dev/null | sed 's|^|    |' >> "$REPORT"
done

if [[ -d "$HOME/.config/autostart" ]]; then
  while read -r f; do
    [[ -z "$f" ]] && continue
    if has_strong "$f"; then
      bad "autostart entry contains an indicator: $f"
      quarantine "$f" "autostart-entry"
    else
      warn "autostart entry present, verify by hand: $f"
    fi
  done < <(find "$HOME/.config/autostart" -maxdepth 1 -name '*.desktop' 2>/dev/null)
else
  ok "no ~/.config/autostart"
fi

local cron; cron="$(crontab -l 2>/dev/null)"
if [[ -n "$cron" ]]; then
  warn "user crontab is not empty, review every line:"
  printf '%s\n' "$cron" | sed 's|^|    |' | tee -a "$REPORT"
else
  ok "user crontab is empty"
fi
for d in /etc/cron.d /etc/cron.daily /etc/cron.hourly; do
  [[ -d "$d" ]] || continue
  while read -r f; do
    [[ -z "$f" ]] && continue
    has_strong "$f" && { bad "system cron entry contains an indicator: $f"; quarantine "$f" "system-cron"; }
  done < <(find "$d" -maxdepth 1 -type f 2>/dev/null)
done
}

check_connections() {
  hdr "Live connections from node and Electron processes"
  local netcmd=""
  command -v ss >/dev/null 2>&1 && netcmd="ss -tnp"
  [[ -z "$netcmd" ]] && command -v netstat >/dev/null 2>&1 && netcmd="netstat -tnp"
  if [[ -n "$netcmd" ]]; then
    check_network "$($netcmd 2>/dev/null | grep -Ei '(node|code|cursor|electron)' | head -40)"
  else
    warn "neither ss nor netstat available, skipped"
  fi
}

run_check "Second-stage implant"        check_implants   "${ROOTS[@]}"
run_host_check "IDE extensions"              check_extensions "$HOME/.vscode/extensions" "$HOME/.vscode-insiders/extensions" \
                                                         "$HOME/.cursor/extensions" "$HOME/.windsurf/extensions" \
                                                         "$HOME/.vscode-oss/extensions" \
                                                         "$HOME/.var/app/com.visualstudio.code/data/vscode/extensions"
run_check "Workspace tasks"             check_tasks_json  "${ROOTS[@]}"
run_check "Build configs"               check_configs     "${ROOTS[@]}"
run_check "Font files"                  check_fonts       "${ROOTS[@]}"
run_host_check "Propagation artifact"        check_propagation "$HOME"
run_check "Known-bad packages"          check_packages    "${ROOTS[@]}"
run_host_check "Persistence"                 check_persistence
run_host_check "Shell startup files"         check_shell_rc "$HOME/.bashrc" "$HOME/.bash_profile" "$HOME/.profile" \
                                                       "$HOME/.zshrc" "$HOME/.zprofile" "$HOME/.zshenv"
run_check "Git configuration and hooks" check_git      "${ROOTS[@]}"
run_host_check "npm configuration"           check_npm
run_host_check "Resident interpreters"       check_processes
run_host_check "Live connections"            check_connections
run_check "Credential surface"          check_credentials "${ROOTS[@]}"
verdict
exit $?
