#!/usr/bin/env bash
# check-linux.sh - PolinRider check and cleanup for Linux.
#
# Default is a dry run: it inspects and reports, and changes nothing.
# --apply moves confirmed artifacts into a quarantine directory. It never deletes.
#
# Usage:
#   ./check-linux.sh [--apply] [--quarantine DIR] [--report FILE] [ROOT ...]
#
# ROOT is a directory holding your code. Defaults to ~/src ~/code ~/dev
# ~/projects ~/work ~/git. Give the real ones, the scan is only as good as its roots.
#
# Exit codes: 0 clean, 1 review items only, 2 confirmed indicator hit.

set -uo pipefail
# shellcheck source=lib/local-common.sh
. "$(cd "$(dirname "$0")" && pwd)/../lib/local-common.sh"

TS="$(date -u +%Y%m%dT%H%M%SZ)"
# shellcheck disable=SC2034  # QDIR and REPORT are read by lib/local-common.sh
QDIR="$HOME/polinrider-quarantine-$TS"
REPORT="$HOME/polinrider-report-$TS.txt"
ROOTS=()

# shellcheck disable=SC2034  # QDIR is read by quarantine() in lib/local-common.sh
while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply)      APPLY=1; shift ;;
    --quarantine) QDIR="$2"; shift 2 ;;
    --report)     REPORT="$2"; shift 2 ;;
    -h|--help)    sed -n '2,16p' "$0"; exit 0 ;;
    -*)           echo "unknown argument: $1" >&2; exit 3 ;;
    *)            ROOTS+=("$1"); shift ;;
  esac
done
[[ ${#ROOTS[@]} -eq 0 ]] && ROOTS=("$HOME/src" "$HOME/code" "$HOME/dev" "$HOME/projects" "$HOME/work" "$HOME/git")

: > "$REPORT"
prc_local_load_iocs
quarantine_init

say "PolinRider local check - Linux - $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "host: $(hostname)   user: $(whoami)"
say "roots: ${ROOTS[*]}"
say "mode: $([[ $APPLY -eq 1 ]] && echo 'APPLY - confirmed artifacts will be moved to quarantine' || echo 'dry run - nothing will be changed')"

check_implants   "${ROOTS[@]}"

check_extensions "$HOME/.vscode/extensions" "$HOME/.vscode-insiders/extensions" \
                 "$HOME/.cursor/extensions" "$HOME/.windsurf/extensions" \
                 "$HOME/.vscode-oss/extensions" \
                 "$HOME/.var/app/com.visualstudio.code/data/vscode/extensions"
check_tasks_json  "${ROOTS[@]}"
check_configs     "${ROOTS[@]}"
check_fonts       "${ROOTS[@]}"
check_propagation "$HOME"
check_packages    "${ROOTS[@]}"

hdr "Persistence: systemd units, autostart, cron"
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

CRON="$(crontab -l 2>/dev/null)"
if [[ -n "$CRON" ]]; then
  warn "user crontab is not empty, review every line:"
  printf '%s\n' "$CRON" | sed 's|^|    |' | tee -a "$REPORT"
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

check_shell_rc "$HOME/.bashrc" "$HOME/.bash_profile" "$HOME/.profile" \
               "$HOME/.zshrc" "$HOME/.zprofile" "$HOME/.zshenv"
check_git      "${ROOTS[@]}"
check_npm

check_processes

hdr "Live connections from node and Electron processes"
NETCMD=""
command -v ss >/dev/null 2>&1 && NETCMD="ss -tnp"
[[ -z "$NETCMD" ]] && command -v netstat >/dev/null 2>&1 && NETCMD="netstat -tnp"
if [[ -n "$NETCMD" ]]; then
  check_network "$($NETCMD 2>/dev/null | grep -Ei '(node|code|cursor|electron)' | head -40)"
else
  warn "neither ss nor netstat available, skipped"
fi

check_credentials "${ROOTS[@]}"
verdict
exit $?
