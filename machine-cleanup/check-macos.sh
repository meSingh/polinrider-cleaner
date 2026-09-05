#!/usr/bin/env bash
# check-macos.sh - PolinRider check and cleanup for macOS.
#
# Default is a dry run: it inspects and reports, and changes nothing.
# --apply moves confirmed artifacts into a quarantine directory. It never deletes.
#
# Usage:
#   ./check-macos.sh [--apply] [--quarantine DIR] [--report FILE] [ROOT ...]
#
# ROOT is a directory holding your code. Defaults to ~/Sites ~/Projects ~/code
# ~/dev ~/Documents. Give the real ones, the scan is only as good as its roots.
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
    -*)           echo "unknown argument: $1" >&2; exit 2 ;;
    *)            ROOTS+=("$1"); shift ;;
  esac
done
[[ ${#ROOTS[@]} -eq 0 ]] && ROOTS=("$HOME/Sites" "$HOME/Projects" "$HOME/code" "$HOME/dev" "$HOME/Documents")

: > "$REPORT"
prc_local_load_iocs
quarantine_init

say "PolinRider local check - macOS - $(date -u +%Y-%m-%dT%H:%M:%SZ)"
say "host: $(hostname)   user: $(whoami)"
say "roots: ${ROOTS[*]}"
say "mode: $([[ $APPLY -eq 1 ]] && echo 'APPLY - confirmed artifacts will be moved to quarantine' || echo 'dry run - nothing will be changed')"

check_implants   "${ROOTS[@]}"

check_extensions "$HOME/.vscode/extensions" "$HOME/.vscode-insiders/extensions" \
                 "$HOME/.cursor/extensions" "$HOME/.windsurf/extensions" \
                 "$HOME/.vscode-oss/extensions"
check_tasks_json  "${ROOTS[@]}"
check_configs     "${ROOTS[@]}"
check_fonts       "${ROOTS[@]}"
check_propagation "$HOME"
check_packages    "${ROOTS[@]}"

hdr "Persistence: LaunchAgents, LaunchDaemons, cron"
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
done
CRON="$(crontab -l 2>/dev/null)"
if [[ -n "$CRON" ]]; then
  warn "user crontab is not empty, review every line:"
  printf '%s\n' "$CRON" | sed 's|^|    |' | tee -a "$REPORT"
else
  ok "user crontab is empty"
fi

check_shell_rc "$HOME/.zshrc" "$HOME/.zprofile" "$HOME/.zshenv" \
               "$HOME/.bashrc" "$HOME/.bash_profile" "$HOME/.profile"
check_git      "${ROOTS[@]}"
check_npm

check_processes

hdr "Live connections from node and Electron processes"
if command -v lsof >/dev/null 2>&1; then
  check_network "$(lsof -nP -iTCP -sTCP:ESTABLISHED 2>/dev/null | grep -Ei '(node|Code Helper|Cursor|Electron)' | head -40)"
else
  warn "lsof not available, skipped"
fi

check_credentials "${ROOTS[@]}"
verdict
exit $?
