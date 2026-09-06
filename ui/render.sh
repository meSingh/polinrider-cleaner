#!/usr/bin/env bash
# render.sh - everything that draws. Sourced, never executed.
#
# Presentation only. Nothing here reads a repository, runs git, or decides
# whether something is infected. It takes text and makes it legible. Read
# theme.sh for the palette; the tools are in lib/ and ci/.
#
# The layout rule, which is what makes the output readable:
#
#   ui_section   a stage of the work          flush left, ruled
#     ui_step      something being done         2 spaces, coloured mark
#       ui_ok        a result of that step        6 spaces, marked
#       ui_stream    raw output from git etc.     6 spaces, dimmed
#
# Anything a subprocess prints goes through ui_stream, so git chatter can never
# sit at the same visual level as a finding.

PRC_UI_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=ui/theme.sh
. "$PRC_UI_DIR/theme.sh"

# --- the wordmark -----------------------------------------------------------
ui_banner() {
  if [[ "$PRC_UNICODE" -eq 1 ]]; then
    printf '%s' "$C_CYAN"
    printf '%s\n' \
'  ┌─┐┌─┐┬  ┬┌┐┌┬─┐┬┌┬┐┌─┐┬─┐' \
'  ├─┘│ ││  │││││├┬┘│ │││├┤ ├┬┘' \
'  ┴  └─┘┴─┘┴┘└┘┴└─┴─┴┘└─┘┴└─'
    printf '%s' "$C_RESET"
  else
    printf '%s  P O L I N R I D E R%s\n' "$C_CYAN" "$C_RESET"
  fi
  printf '%s  cleaner %s  %s\n\n' "$C_DIM" "${PRC_VERSION:-}" "$C_RESET"
}

ui_rule() {
  local w="${1:-$PRC_COLS}" out="" i=0
  while [[ $i -lt $w ]]; do out="$out$S_RULE"; i=$((i+1)); done
  printf '%s%s%s\n' "$C_DIM" "$out" "$C_RESET"
}

# --- structure --------------------------------------------------------------
ui_section() { printf '\n%s%s%s\n' "$C_BOLD" "$*" "$C_RESET"; ui_rule; }
ui_step()    { printf '\n%s  %s %s%s\n' "$T_STEP" "$T_STEP_S" "$*" "$C_RESET"; }
ui_blank()   { printf '\n'; }
ui_text()    { printf '  %s\n' "$*"; }
ui_dim()     { printf '%s  %s%s\n' "$C_DIM" "$*" "$C_RESET"; }

# --- results, always the same colour for the same meaning -------------------
ui_ok()     { printf '      %s%s%s %s\n' "$T_OK"     "$T_OK_S"     "$C_RESET" "$*"; }
ui_bad()    { printf '      %s%s %s%s\n' "$T_HIT"    "$T_HIT_S"    "$*" "$C_RESET"; }
ui_warn()   { printf '      %s%s%s %s\n' "$T_REVIEW" "$T_REVIEW_S" "$C_RESET" "$*"; }
ui_info()   { printf '      %s%s %s%s\n' "$T_INFO"   "$T_INFO_S"   "$*" "$C_RESET"; }
ui_bullet() { printf '      %s %s\n' "$S_DOT" "$*"; }

# ui_kv <key> <value> - aligned pairs for a summary
ui_kv() { printf '      %s%-26s%s %s\n' "$C_DIM" "$1" "$C_RESET" "$2"; }

# --- subprocess output ------------------------------------------------------
# ui_stream - a pipe filter. Indents, dims, and drops the [HH:MM:SS] prefix the
# libraries emit, because the step above already says what is happening. This
# is what stops git output reading as though it were a finding.
ui_stream() {
  local line w=$((PRC_COLS - 8))
  while IFS= read -r line || [[ -n "$line" ]]; do
    line="${line#\[??:??:??\] }"
    case "$line" in '['[0-9][0-9]:[0-9][0-9]:[0-9][0-9]'] '*) line="${line#*] }" ;; esac
    [[ -z "$line" ]] && continue
    [[ "${#line}" -gt "$w" ]] && line="${line:0:$w}…"
    printf '%s      %s%s\n' "$C_GREY" "$line" "$C_RESET"
  done
}


# ui_findings - a pipe filter for scanner output. Unlike ui_stream, which dims
# everything, this colours by meaning, so a confirmed hit cannot look like a
# progress message. It understands both output shapes in this repository: the
# workspace scanner's "INFECTED path:line" and the machine check's "[HIT]".
# Anything it does not recognise is dimmed, which is the safe default: unknown
# text never borrows the colour of a finding.
ui_findings() {
  local line body w=$((PRC_COLS - 8))
  _trim() { local v="$1"; while [[ "$v" == [[:space:]]* ]]; do v="${v#?}"; done; printf '%s' "$v"; }
  while IFS= read -r line || [[ -n "$line" ]]; do
    case "$line" in '['[0-9][0-9]:[0-9][0-9]:[0-9][0-9]'] '*) line="${line#*] }" ;; esac
    [[ -z "$line" ]] && { printf '\n'; continue; }
    case "$line" in
      INFECTED*)          body="$(_trim "${line#INFECTED}")"
                          printf '      %s%s %s%s\n' "$T_HIT" "$T_HIT_S" "$body" "$C_RESET" ;;
      *'[HIT]'*)          body="$(_trim "${line#*\[HIT\]}")"
                          printf '      %s%s %s%s\n' "$T_HIT" "$T_HIT_S" "$body" "$C_RESET" ;;
      review*)            body="$(_trim "${line#review}")"
                          printf '      %s%s%s %s\n' "$T_REVIEW" "$T_REVIEW_S" "$C_RESET" "$body" ;;
      *'[review]'*)       body="$(_trim "${line#*\[review\]}")"
                          printf '      %s%s%s %s\n' "$T_REVIEW" "$T_REVIEW_S" "$C_RESET" "$body" ;;
      *'[ok]'*)           body="$(_trim "${line#*\[ok\]}")"
                          printf '      %s%s%s %s\n' "$T_OK" "$T_OK_S" "$C_RESET" "$body" ;;
      *'[info]'*)         body="$(_trim "${line#*\[info\]}")"
                          printf '      %s%s %s%s\n' "$T_INFO" "$T_INFO_S" "$body" "$C_RESET" ;;
      clean*)             continue ;;
      '===='*|' ===='*)   continue ;;                 # the engines' own rules
      '=='*'=='*)         body="${line#== }"; body="${body%% ==}"
                          printf '\n    %s%s%s\n' "$C_BOLD" "$body" "$C_RESET" ;;
      *)                  [[ "${#line}" -gt "$w" ]] && line="${line:0:$w}…"
                          printf '%s      %s%s\n' "$C_GREY" "$line" "$C_RESET" ;;
    esac
  done
}

# --- progress ---------------------------------------------------------------
# On a terminal this rewrites one line. Anywhere else it prints nothing until
# ui_progress_done, so a CI log or a pipe does not fill with control codes.
ui_progress() {  # ui_progress <current> <total> [label]
  local cur="$1" total="$2" label="${3:-}" width=28 filled bar="" empty=""
  [[ "$PRC_TTY" -eq 1 ]] || return 0
  [[ "${total:-0}" -gt 0 ]] || return 0
  filled=$(( cur * width / total ))
  local i=0
  while [[ $i -lt $filled ]]; do bar="$bar$S_BAR_FULL"; i=$((i+1)); done
  while [[ $i -lt $width ]];  do empty="$empty$S_BAR_EMPTY"; i=$((i+1)); done
  local pct=$(( cur * 100 / total )) max=$((PRC_COLS - 48))
  [[ "${#label}" -gt "$max" && "$max" -gt 3 ]] && label="${label:0:$max}…"
  printf '%s      %s%s%s%s %3s%%  %s%-*s%s' "$C_ERASE" \
    "$C_CYAN" "$bar" "$C_DIM" "$empty" "$pct" "$C_GREY" "$max" "$label" "$C_RESET"
}
ui_progress_done() {  # ui_progress_done <total> <noun>
  [[ "$PRC_TTY" -eq 1 ]] && printf '%s' "$C_ERASE"
  ui_ok "$1 $2"
}

# --- asking -----------------------------------------------------------------
# ui_ask <question> [y|n] - 0 for yes. PRC_YES=1 answers yes without asking.
ui_ask() {
  local q="$1" def="${2:-n}" hint reply
  [[ "${PRC_YES:-0}" -eq 1 ]] && return 0
  if [[ "$def" == "y" ]]; then hint="[Y/n]"; else hint="[y/N]"; fi
  printf '\n  %s%s%s %s%s%s ' "$T_ASK" "$q" "$C_RESET" "$C_DIM" "$hint" "$C_RESET"
  read -r reply || return 1
  reply="${reply:-$def}"
  case "$reply" in [Yy]*) return 0 ;; *) return 1 ;; esac
}

# ui_menu <title> <label>... - prints a numbered menu, echoes the chosen index.
# Returns 1 if the operator declines to choose.
ui_menu() {
  local title="$1"; shift
  local n=$# i=1 reply
  printf '\n  %s%s%s\n\n' "$C_BOLD" "$title" "$C_RESET"
  for opt in "$@"; do
    printf '    %s%s%s  %s\n' "$C_CYAN" "$i" "$C_RESET" "$opt"
    i=$((i+1))
  done
  printf '\n  %sChoose 1-%s, or q to stop:%s ' "$C_BOLD" "$n" "$C_RESET"
  read -r reply || return 1
  case "$reply" in
    [Qq]*|"") return 1 ;;
    *[!0-9]*) return 1 ;;
  esac
  [[ "$reply" -ge 1 && "$reply" -le "$n" ]] || return 1
  printf '%s\n' "$reply"
}

# --- verdict panel ----------------------------------------------------------
# ui_verdict <hit|review|clean|error> <headline>
ui_verdict() {
  local kind="$1"; shift
  local col mark
  case "$kind" in
    hit)    col="$T_HIT";    mark="$T_HIT_S" ;;
    review) col="$T_REVIEW"; mark="$T_REVIEW_S" ;;
    clean)  col="$T_OK";     mark="$T_OK_S" ;;
    *)      col="$C_YELLOW"; mark="$S_WARN" ;;
  esac
  printf '\n%s%s %s%s\n' "$col$C_BOLD" "$mark" "$*" "$C_RESET"
}
