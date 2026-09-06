#!/usr/bin/env bash
# theme.sh - colours and symbols. Sourced, never executed.
#
# This file decides what things LOOK like and nothing else. No scanning logic
# lives here, so you can read it in a minute and satisfy yourself that the
# presentation layer cannot change what the tools do. The drawing functions are
# in render.sh; the tools themselves are in lib/ and ci/.
#
# Everything here can be overridden from the environment before sourcing.
# Honoured automatically:
#   NO_COLOR=1        no colour, any value          https://no-color.org
#   PRC_ASCII=1       plain ASCII symbols only
#   TERM=dumb         treated as no colour, no cursor tricks
#   not a terminal    piping or redirecting turns everything off

# --- what can this terminal do? ---------------------------------------------
PRC_TTY=0;   [[ -t 1 ]] && PRC_TTY=1
PRC_COLOR=0
if [[ "$PRC_TTY" -eq 1 && -z "${NO_COLOR:-}" && "${TERM:-}" != "dumb" ]]; then PRC_COLOR=1; fi
PRC_UNICODE=0
if [[ -z "${PRC_ASCII:-}" ]]; then
  case "${LC_ALL:-${LC_CTYPE:-${LANG:-}}}" in *[Uu][Tt][Ff]*8*|*[Uu][Tt][Ff]8*) PRC_UNICODE=1 ;; esac
fi
PRC_COLS=80
if [[ "$PRC_TTY" -eq 1 ]]; then
  PRC_COLS="$( (tput cols) 2>/dev/null || echo 80 )"
  [[ "$PRC_COLS" =~ ^[0-9]+$ ]] || PRC_COLS=80
  [[ "$PRC_COLS" -lt 60 ]] && PRC_COLS=60
  [[ "$PRC_COLS" -gt 100 ]] && PRC_COLS=100
fi

# --- palette ----------------------------------------------------------------
# Restrained on purpose. Colour carries meaning here, so the fewer colours in
# play, the more each one means. Red is only ever a confirmed finding.
if [[ "$PRC_COLOR" -eq 1 ]]; then
  C_RESET=$'\033[0m';  C_BOLD=$'\033[1m';   C_DIM=$'\033[2m'
  C_RED=$'\033[31m';   C_GREEN=$'\033[32m'; C_YELLOW=$'\033[33m'
  C_BLUE=$'\033[34m';  C_CYAN=$'\033[36m';  C_GREY=$'\033[90m'
  C_HIDE=$'\033[?25l'; C_SHOW=$'\033[?25h'; C_ERASE=$'\033[2K\r'
else
  C_RESET=""; C_BOLD=""; C_DIM=""; C_RED=""; C_GREEN=""; C_YELLOW=""
  C_BLUE=""; C_CYAN=""; C_GREY=""; C_HIDE=""; C_SHOW=""; C_ERASE=$'\r'
fi

# --- symbols ----------------------------------------------------------------
# Every one has an ASCII twin, so the output is readable over a serial console,
# in a CI log, or on a terminal with no font for box drawing.
if [[ "$PRC_UNICODE" -eq 1 ]]; then
  S_OK="✓"; S_BAD="✗"; S_WARN="!"; S_INFO="i"; S_STEP="▸"; S_DOT="·"
  S_ARROW="→"; S_BAR_FULL="█"; S_BAR_EMPTY="░"; S_RULE="─"
  S_TL="╭"; S_TR="╮"; S_BL="╰"; S_BR="╯"; S_V="│"
else
  S_OK="[ok]"; S_BAD="[!!]"; S_WARN="[..]"; S_INFO="[--]"; S_STEP=">"; S_DOT="."
  S_ARROW="->"; S_BAR_FULL="#"; S_BAR_EMPTY="."; S_RULE="-"
  S_TL="+"; S_TR="+"; S_BL="+"; S_BR="+"; S_V="|"
fi

# --- meaning, not decoration ------------------------------------------------
# Bound once here so a finding never renders as anything but red, anywhere.
T_HIT="$C_RED";     T_HIT_S="$S_BAD"      # a confirmed indicator
T_REVIEW="$C_YELLOW"; T_REVIEW_S="$S_WARN" # needs a person to look
T_OK="$C_GREEN";    T_OK_S="$S_OK"        # checked and clean
T_INFO="$C_GREY";   T_INFO_S="$S_INFO"    # inventory, not a finding
T_STEP="$C_CYAN";   T_STEP_S="$S_STEP"    # a stage of the run
T_ASK="$C_BOLD"                            # a question for the operator
