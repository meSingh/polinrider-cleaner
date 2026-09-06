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

# --- the wordmark ----------------------------------------------------------
# Three sizes. The full one needs 73 columns, so a narrow terminal gets plain
# bold text rather than art squeezed until it stops being letters. That was the
# first version's mistake: ten glyphs in twenty-six columns is not a wordmark.
PRC_BANNER_W=75   # 73 glyphs plus the two-space margin

ui_banner() {
  if [[ "$PRC_UNICODE" -eq 1 && "$PRC_COLS" -ge "$PRC_BANNER_W" ]]; then
    printf '%s' "$C_CYAN"
    printf '%s\n' \
'  ██████╗  ██████╗ ██╗     ██╗███╗   ██╗██████╗ ██╗██████╗ ███████╗██████╗ ' \
'  ██╔══██╗██╔═══██╗██║     ██║████╗  ██║██╔══██╗██║██╔══██╗██╔════╝██╔══██╗' \
'  ██████╔╝██║   ██║██║     ██║██╔██╗ ██║██████╔╝██║██║  ██║█████╗  ██████╔╝'
    printf '%s' "$C_BLUE"
    printf '%s\n' \
'  ██╔═══╝ ██║   ██║██║     ██║██║╚██╗██║██╔══██╗██║██║  ██║██╔══╝  ██╔══██╗' \
'  ██║     ╚██████╔╝███████╗██║██║ ╚████║██║  ██║██║██████╔╝███████╗██║  ██║' \
'  ╚═╝      ╚═════╝ ╚══════╝╚═╝╚═╝  ╚═══╝╚═╝  ╚═╝╚═╝╚═════╝ ╚══════╝╚═╝  ╚═╝'
    printf '%s' "$C_RESET"
  else
    printf '\n  %sPOLINRIDER%s\n' "$C_CYAN$C_BOLD" "$C_RESET"
  fi
  ui_signature
  printf '\n  %scleaner %s%s  %s%s%s\n' \
    "$C_BOLD" "${PRC_VERSION:-}" "$C_RESET" "$C_DIM" "read-only until you say otherwise" "$C_RESET"
}

# ui_link <url> <text> - clickable where the terminal supports OSC 8, plain
# text everywhere else. Never prints the URL, so piped output stays clean.
ui_link() {
  if [[ "$PRC_LINKS" -eq 1 ]]; then
    printf '\033]8;;%s\033\\%s\033]8;;\033\\' "$1" "$2"
  else
    printf '%s' "$2"
  fi
}

# ui_signature - the author credit, right-aligned to the wordmark's edge so it
# reads as a signature under it rather than another line of output.
ui_signature() {
  local text="by $PRC_AUTHOR" pad
  [[ "$PRC_LINKS" -eq 1 && -n "$S_LINK" ]] && text="$text $S_LINK"
  pad=$(( PRC_BANNER_W - ${#text} ))
  [[ "$pad" -lt 2 ]] && pad=2
  printf '%*s%s' "$pad" "" "$C_DIM"
  ui_link "$PRC_AUTHOR_URL" "$text"
  printf '%s\n' "$C_RESET"
}

# ui_file <path> [label] - a short, clickable file reference. A /var/folders
# path tells nobody anything and does not fit; the label names the variable and
# the link opens the real file.
ui_file() {
  local path="$1" label="${2:-}"
  if [[ -z "$label" ]]; then
    if [[ -n "${TMPDIR:-}" && "$path" == "${TMPDIR%/}"/* ]]; then
      label="\$TMPDIR/${path#"${TMPDIR%/}"/}"
    else
      label="${path/#$HOME/~}"
    fi
  fi
  ui_link "file://$path" "$label"
}

# ui_file_line <label> <path> - one aligned row of a file listing.
ui_file_line() {
  printf '      %s%-14s%s ' "$C_DIM" "$1" "$C_RESET"
  ui_file "$2"
  printf '\n'
}

# ui_status <text> - a bullet under the wordmark. The caller gathers the facts;
# nothing in ui/ runs a command to find them out.
ui_status() { printf '  %s%s%s %s\n' "$C_CYAN" "$S_BULLET" "$C_RESET" "$*"; }

ui_rule() {
  local out="" i=0
  while [[ $i -lt $PRC_COLS ]]; do out="$out$S_RULE"; i=$((i+1)); done
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


# _shorten - collapse the long absolute paths the engines print. A
# /var/folders/... prefix is 55 characters that tell nobody anything, and it
# pushed the useful part of the line off the right edge.
_shorten() {
  local t="$1" tp tilde='~' 
  # The prefix has to be computed first: bash cannot nest ${TMPDIR%/} inside a
  # ${t//...} replacement.
  if [[ -n "${TMPDIR:-}" ]]; then
    tp="${TMPDIR%/}/"
    t="${t//"$tp"/\$TMPDIR/}"
  fi
  t="${t//"$HOME"/$tilde}"
  printf '%s' "$t"
}

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
    line="$(_shorten "$line")"
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
    line="$(_shorten "$line")"
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
      *'findings :'*|*'findings  :'*|*'refs scanned'*|*'INFECTED '*[0-9]|*'review '*[0-9])
                          body="$(_trim "$line")"
                          case "$body" in
                            *': 0'|*': 0 '*) printf '      %s%s%s\n' "$C_DIM" "$body" "$C_RESET" ;;
                            *infected*|*INFECTED*)
                                printf '      %s%s%s\n' "$T_HIT$C_BOLD" "$body" "$C_RESET" ;;
                            *review*)
                                printf '      %s%s%s\n' "$T_REVIEW" "$body" "$C_RESET" ;;
                            *)  printf '      %s\n' "$body" ;;
                          esac ;;
      *'PolinRider'*scan*|*'triage complete'*)
                          printf '\n    %s%s%s\n' "$C_BOLD" "$(_trim "$line")" "$C_RESET" ;;
      '===='*|' ===='*)   continue ;;                 # the engines' own rules
      '=='*'=='*)         body="${line#== }"; body="${body%% ==}"
                          printf '\n    %s%s%s\n' "$C_BOLD" "$body" "$C_RESET" ;;
      *)                  body="$(_trim "$line")"
                          if [[ "${#body}" -gt "$w" ]]; then
                            # wrap rather than truncate: these are sentences
                            printf '%s' "$C_GREY"
                            printf '%s\n' "$body" | fold -s -w "$w" | sed 's/^/      /'
                            printf '%s' "$C_RESET"
                          else
                            printf '%s      %s%s\n' "$C_GREY" "$body" "$C_RESET"
                          fi ;;
    esac
  done
}


# ui_pager <file> [lines-per-page] - show a file a page at a time.
#
# Not less, and not $PAGER. Someone who has never used a terminal pager cannot
# guess how to leave one, and on a machine where $PAGER is vi they are properly
# stuck. This reads with the shell alone and says on every page exactly which
# key does what.
ui_pager() {
  local file="$1" per="${2:-24}" line n=0 total shown=0 reply
  [[ -r "$file" ]] || { ui_warn "cannot read $file"; return 1; }
  total=0; while IFS= read -r line || [[ -n "$line" ]]; do total=$((total+1)); done < "$file"
  [[ "$total" -eq 0 ]] && { ui_dim "$file is empty"; return 0; }

  # Paging into a pipe or a log helps nobody: print it and be done.
  if [[ "$PRC_TTY" -ne 1 ]]; then cat "$file"; return 0; fi
  # A controlling terminal is the right place to read a keypress from, but it is
  # not always there. Fall back to stdin rather than failing.
  local tty_in="/dev/tty"; [[ -r "$tty_in" ]] || tty_in="/dev/stdin"

  while IFS= read -r line || [[ -n "$line" ]]; do
    printf '%s\n' "$line"
    n=$((n+1)); shown=$((shown+1))
    if [[ "$n" -ge "$per" && "$shown" -lt "$total" ]]; then
      n=0
      printf '\n  %s%s of %s lines. %sEnter%s for more, %sq%s then Enter to stop: ' \
        "$C_DIM" "$shown" "$total" "$C_RESET$C_BOLD" "$C_DIM" "$C_BOLD" "$C_RESET$C_DIM"
      printf '%s' "$C_RESET"
      read -r reply <"$tty_in" || return 0
      case "$reply" in [Qq]*) ui_blank; ui_dim "stopped at line $shown of $total"; return 0 ;; esac
    fi
  done < "$file"
  ui_blank
  ui_dim "end of file, $total lines"
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

# ui_prompt <label> [hint] - reads one line and echoes it, empty or not. Returns
# 1 only at end of input. Whether a blank answer means "skip" or "ask me again"
# is the caller's decision, not this function's.
ui_prompt() {
  local label="$1" hint="${2:-}" reply
  if [[ -n "$hint" ]]; then
    printf '\n  %s%s%s %s%s%s ' "$C_BOLD" "$label" "$C_RESET" "$C_DIM" "$hint" "$C_RESET" >&2
  else
    printf '\n  %s%s%s ' "$C_BOLD" "$label" "$C_RESET" >&2
  fi
  read -r reply || return 1
  printf '%s\n' "$reply"
}

# ui_choice <question> <key> <label> [<key> <label>]... - a keyed question.
# Echoes the chosen key. For answers that are not simply yes or no.
ui_choice() {
  local q="$1"; shift
  local -a keys=() labels=()
  while [[ $# -ge 2 ]]; do keys+=("$1"); labels+=("$2"); shift 2; done
  local i reply hint=""
  for i in "${!keys[@]}"; do hint="$hint${hint:+/}${keys[$i]}"; done
  {
    printf '\n  %s%s%s\n' "$C_BOLD" "$q" "$C_RESET"
    for i in "${!keys[@]}"; do
      printf '      %s%s%s  %s\n' "$C_CYAN" "${keys[$i]}" "$C_RESET" "${labels[$i]}"
    done
  } >&2
  while :; do
    printf '\n  %s[%s]:%s ' "$C_BOLD" "$hint" "$C_RESET" >&2
    read -r reply || return 1
    for i in "${!keys[@]}"; do
      [[ "$reply" == "${keys[$i]}" ]] && { printf '%s\n' "$reply"; return 0; }
    done
    printf '  %sanswer with one of: %s%s\n' "$C_DIM" "$hint" "$C_RESET" >&2
  done
}

# ui_menu <title> <label>... - prints a numbered menu, echoes the chosen index.
#
# Exit codes, because "leave this menu" and "leave the tool" are different
# things and were the same key:
#   0  a choice was made, on stdout
#   1  go back one level      (b, offered only when PRC_MENU_BACK=1)
#   2  quit the tool entirely (q)
#   3  no input at all        (end of input)
#
# 2 and 3 are separate because "the operator chose to stop" and "nobody was
# there to answer" deserve different exit codes from the tool.
ui_menu() {
  local title="$1"; shift
  # An option may be shown but not offered. Prefix it with "!" to render it
  # dimmed with a dash instead of a number, and "~" for a continuation line
  # explaining why. Neither is selectable, and neither consumes a number, so a
  # caller's action list still lines up with what the operator can pick.
  local n=0 i=1 reply opt
  for opt in "$@"; do
    case "$opt" in '!'*|'~'*) ;; *) n=$((n+1)) ;; esac
  done
  printf '\n  %s%s%s\n\n' "$C_BOLD" "$title" "$C_RESET" >&2
  for opt in "$@"; do
    case "$opt" in
      '!'*) printf '    %s-  %s%s\n' "$C_DIM" "${opt#!}" "$C_RESET" >&2 ;;
      '~'*) printf '       %s%s%s\n' "$C_DIM" "${opt#\~}" "$C_RESET" >&2 ;;
      *)    printf '    %s%s%s  %s\n' "$C_CYAN" "$i" "$C_RESET" "$opt" >&2
            i=$((i+1)) ;;
    esac
  done
  # Only q and end-of-input leave. A bare Enter, a typo or a number out of range
  # re-asks. Treating an empty line as "stop" meant one stray keypress after the
  # pager ended dropped the operator out of the whole run.
  local back_hint=""
  [[ "${PRC_MENU_BACK:-0}" -eq 1 ]] && back_hint="b to go back, "
  while :; do
    printf '\n  %sChoose 1-%s, %sq to quit:%s ' "$C_BOLD" "$n" "$back_hint" "$C_RESET" >&2
    read -r reply || return 3
    case "$reply" in
      [Qq]*)    return 2 ;;
      [Bb]*)    [[ "${PRC_MENU_BACK:-0}" -eq 1 ]] && return 1
                printf '  %snothing to go back to. Pick 1 to %s, or q to quit.%s\n' \
                  "$C_DIM" "$n" "$C_RESET" >&2; continue ;;
      "")       continue ;;
      *[!0-9]*) printf '  %snot a number. Pick 1 to %s, or q to quit.%s\n' "$C_DIM" "$n" "$C_RESET" >&2
                continue ;;
    esac
    if [[ "$reply" -ge 1 && "$reply" -le "$n" ]]; then printf '%s\n' "$reply"; return 0; fi
    printf '  %sthere is no option %s. Pick 1 to %s, or q to quit.%s\n' "$C_DIM" "$reply" "$n" "$C_RESET" >&2
  done
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
