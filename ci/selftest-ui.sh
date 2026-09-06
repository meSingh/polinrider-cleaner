#!/usr/bin/env bash
# selftest-ui.sh - the presentation layer, offline.
#
# Two things are being protected here. That the output degrades correctly, so a
# CI log or a pipe never fills with escape codes. And that ui/ stays
# presentation only, so auditing this tool never requires reading it.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/prc-ui.XXXXXX")"
trap 'rm -rf "$TMP"' EXIT
PASS=0; FAIL=0
ok(){ PASS=$((PASS+1)); printf '  ok   %s\n' "$*"; }
no(){ FAIL=$((FAIL+1)); printf '  FAIL %s\n' "$*"; }
esc=$'\033'

echo "== it turns itself off =="
out="$(. "$ROOT/ui/render.sh"; ui_section X; ui_ok Y; ui_bad Z 2>&1)"
[[ "$out" != *"$esc"* ]] && ok "no escape codes when stdout is not a terminal" \
                         || no "escape codes leaked into a pipe"
out="$(NO_COLOR=1 bash -c '. '"$ROOT"'/ui/render.sh; ui_bad hit')"
[[ "$out" != *"$esc"* ]] && ok "NO_COLOR is honoured" || no "NO_COLOR is honoured"
out="$(TERM=dumb bash -c '. '"$ROOT"'/ui/render.sh; ui_bad hit')"
[[ "$out" != *"$esc"* ]] && ok "TERM=dumb is honoured" || no "TERM=dumb is honoured"
out="$(PRC_ASCII=1 LANG=en_US.UTF-8 bash -c '. '"$ROOT"'/ui/render.sh; ui_ok fine')"
[[ "$out" == *"[ok]"* ]] && ok "PRC_ASCII falls back to ASCII marks" || no "PRC_ASCII falls back to ASCII marks"
out="$(LANG=C bash -c '. '"$ROOT"'/ui/render.sh; ui_banner')"
[[ "$out" != *"█"* && "$out" == *POLINRIDER* ]] \
  && ok "a non-UTF-8 locale gets plain text, still legible" \
  || no "a non-UTF-8 locale gets plain text, still legible"
out="$(LANG=en_US.UTF-8 bash -c '. '"$ROOT"'/ui/render.sh; PRC_COLS=60 ui_banner')"
[[ "$out" != *"█"* && "$out" == *POLINRIDER* ]] \
  && ok "a narrow terminal gets plain text rather than squashed art" \
  || no "a narrow terminal gets plain text rather than squashed art"
# The wordmark must fit the width it claims to need, or it wraps into nonsense.
out="$(LANG=en_US.UTF-8 bash -c '. '"$ROOT"'/ui/render.sh; PRC_COLS=80 ui_banner' | sed -n 2p)"
# wc counts bytes unless it is told the encoding; these glyphs are 3 bytes each.
w=$(printf '%s' "$out" | LC_ALL=en_US.UTF-8 wc -m | tr -d ' ')
[[ "$w" -le 80 ]] && ok "the wordmark fits in 80 columns ($w)" || no "the wordmark is $w columns wide"

echo "== the author signature =="
out="$(LANG=en_US.UTF-8 bash -c '. '"$ROOT"'/ui/render.sh; ui_banner')"
[[ "$out" == *"by Mandeep Singh"* ]] && ok "the signature is present" || no "the signature is present"
# Piped output must stay clean: no OSC 8, and no bare URL leaking into a log.
[[ "$out" != *"]8;;"* ]] && ok "no hyperlink escape when stdout is not a terminal" \
                         || no "no hyperlink escape when stdout is not a terminal"
[[ "$out" != *"https://"* ]] && ok "the URL is not printed as text" || no "the URL is not printed as text"
# Right-aligned to the wordmark, so it reads as a signature under it.
sigline="$(printf '%s\n' "$out" | grep -n 'by Mandeep Singh' | cut -d: -f2-)"
w=$(printf '%s' "$sigline" | LC_ALL=en_US.UTF-8 wc -m | tr -d ' ')
[[ "$w" -eq 75 ]] && ok "the signature is flush with the wordmark's right edge" \
                  || no "the signature ends at column $w, wordmark ends at 75"
out="$(PRC_AUTHOR='Someone Else' PRC_AUTHOR_URL='https://example.invalid' \
       bash -c '. '"$ROOT"'/ui/render.sh; ui_banner')"
[[ "$out" == *"by Someone Else"* ]] && ok "a fork can rebrand it from the environment" \
                                    || no "a fork can rebrand it from the environment"

echo "== findings are classified, not just dimmed =="
cls() { printf '%s\n' "$1" | ( . "$ROOT/ui/render.sh"; ui_findings ); }
[[ "$(cls 'INFECTED a.js:1:bad')" == *"a.js:1:bad"* ]] && ok "INFECTED lines survive" || no "INFECTED lines survive"
[[ "$(cls '  [HIT]    implant found')" == *"implant found"* ]] && ok "[HIT] lines survive" || no "[HIT] lines survive"
[[ "$(cls 'review    b.js:2:maybe')" == *"b.js:2:maybe"* ]] && ok "review lines survive" || no "review lines survive"
# The engines pad their labels ("review    path"). That padding must be trimmed,
# or the body starts at a different column depending on the label.
a="$(cls 'INFECTED      x')"; b="$(cls 'review        x')"
[[ "$a" == *' x' && "$a" != *'  x' && "$b" == *' x' && "$b" != *'  x' ]] \
  && ok "label padding is trimmed from the body" || no "label padding is trimmed: got [$a] [$b]"
[[ -z "$(cls '[13:27:22] cloning something')" ]] || \
  [[ "$(cls '[13:27:22] cloning something')" != *"13:27:22"* ]] \
  && ok "the timestamp prefix is stripped" || no "the timestamp prefix is stripped"
[[ -z "$(cls 'clean   nothing here')" ]] && ok "clean verdicts are not printed one by one" \
                                         || no "clean verdicts are not printed one by one"

echo "== the pager cannot trap anyone =="
printf 'line %s\n' 1 2 3 4 5 6 7 8 9 10 > "$TMP/pg.txt"
out="$(. "$ROOT/ui/render.sh"; ui_pager "$TMP/pg.txt" 3)"
[[ "$(printf '%s\n' "$out" | grep -c '^line')" -eq 10 ]] \
  && ok "off a terminal it prints the file whole, no prompts" \
  || no "off a terminal it prints the file whole, no prompts"
[[ "$out" != *"Enter for more"* ]] && ok "no paging prompt when output is piped" \
                                   || no "no paging prompt when output is piped"
grep -q 'q%s then Enter to stop' "$ROOT/ui/render.sh" \
  && ok "the prompt says which key leaves it" || no "the prompt says which key leaves it"
# $PAGER is never invoked: on a machine where it is vi, that traps people.
# Comments about not using it are fine, so strip those before looking.
if grep -h '' "$ROOT/polinrider.sh" "$ROOT/ui/render.sh" \
     | sed 's/^[[:space:]]*//' | grep -v '^#' \
     | grep -qE '\$\{?PAGER|(^|[^a-z-])less '; then
  no "no reliance on \$PAGER or less"
else ok "no reliance on \$PAGER or less"; fi
out="$(. "$ROOT/ui/render.sh"; ui_pager "$TMP/does-not-exist" 2>&1)"
[[ "$out" == *"cannot read"* ]] && ok "a missing file is reported, not crashed on" \
                               || no "a missing file is reported, not crashed on"

echo "== long operations announce themselves =="
grep -q 'clone --bare --progress' "$ROOT/lib/gh-clean.sh" \
  && ok "the bare clone shows git progress" || no "the bare clone shows git progress"
grep -q 'clone --bare --quiet' "$ROOT/lib/gh-clean.sh" \
  && no "no silent --quiet clone remains" || ok "no silent --quiet clone remains"
grep -q 'MB, nothing is checked out' "$ROOT/lib/gh-clean.sh" \
  && ok "the repository size is stated before the wait" || no "the repository size is stated before the wait"

echo "== ui_stream indents subprocess output =="
out="$(printf 'remote: Counting\n' | ( . "$ROOT/ui/render.sh"; ui_stream ))"
[[ "$out" == "      "* ]] && ok "subprocess output is indented under its step" || no "subprocess output is indented"

echo "== guidance is hidden when driven, not deleted =="
for f in "$ROOT/lib/gh-scan.sh" "$ROOT/lib/triage-filter.sh" "$ROOT/lib/next-steps.sh"; do
  grep -q 'PRC_EMBEDDED' "$f" && ok "$(basename "$f") checks PRC_EMBEDDED" \
                              || no "$(basename "$f") checks PRC_EMBEDDED"
done
grep -q 'PRC_EMBEDDED=1; export PRC_EMBEDDED' "$ROOT/polinrider.sh" \
  && ok "the entry point sets it" || no "the entry point sets it"
# Standalone must still print it, or auditors lose the instructions.
o1="$("$ROOT/lib/triage-filter.sh" "$TMP/t.json" 2>/dev/null || true)"
printf '[]' > "$TMP/t.json"
o1="$("$ROOT/lib/triage-filter.sh" "$TMP/t.json" 2>/dev/null || true)"
o2="$(PRC_EMBEDDED=1 "$ROOT/lib/triage-filter.sh" "$TMP/t.json" 2>/dev/null || true)"
[[ "$o1" == *"Read every REAL_SUSPECT"* ]] && ok "standalone still prints its guidance" \
                                           || no "standalone still prints its guidance"
[[ "$o2" != *"Read every REAL_SUSPECT"* ]] && ok "embedded suppresses it" || no "embedded suppresses it"

echo "== long paths are shortened, not truncated =="
out="$(printf 'report : %s/x/triage.json\n' "${TMPDIR%/}" | ( . "$ROOT/ui/render.sh"; ui_findings ))"
[[ "$out" == *'$TMPDIR/x/triage.json'* ]] && ok "a \$TMPDIR path is collapsed to the variable" \
                                          || no "a \$TMPDIR path is collapsed: got [$out]"
out="$(printf 'cloning %s/w\n' "$HOME" | ( . "$ROOT/ui/render.sh"; ui_stream ))"
[[ "$out" == *"~/w"* ]] && ok "a \$HOME path is collapsed to a tilde" || no "a \$HOME path is collapsed: got [$out]"

echo "== a captured function returns only its value =="
# This has now been the same bug three times: ui_menu, ui_prompt and pick_repo
# each printed their display on stdout, so a caller using $( ) captured the menu
# and the operator saw nothing.
printf 'a/one\na/two\n' > "$TMP/repos.txt"
out="$(bash -c '. '"$ROOT"'/ui/render.sh
'"$(sed -n '/^pick_repo()/,/^}/p' "$ROOT/polinrider.sh")"'
printf "1\n" | pick_repo '"$TMP"'/repos.txt' 2>/dev/null)"
[[ "$out" == "a/one" ]] && ok "pick_repo returns only the chosen repository" \
                        || no "pick_repo returned [$out]"
out="$(bash -c '. '"$ROOT"'/ui/render.sh; printf "2\n" | ui_menu T a b c' 2>/dev/null)"
[[ "$out" == "2" ]] && ok "ui_menu returns only the chosen number" || no "ui_menu returned [$out]"
out="$(bash -c '. '"$ROOT"'/ui/render.sh; printf "hi\n" | ui_prompt L' 2>/dev/null)"
[[ "$out" == "hi" ]] && ok "ui_prompt returns only the typed line" || no "ui_prompt returned [$out]"

echo "== an unavailable option is shown, not omitted =="
# A missing option and an impossible one look identical from the outside, so a
# disabled entry keeps its place in the list with the reason attached.
out="$(bash -c '. '"$ROOT"'/ui/render.sh
  printf "2\n" | ui_menu T "first" "!Restore branches   unavailable here" "~because of a window" "second"' 2>&1)"
[[ "$out" == *"-  Restore branches"* ]] && ok "a disabled option renders with a dash" \
                                        || no "a disabled option renders with a dash"
[[ "$out" == *"because of a window"* ]] && ok "its reason is shown with it" || no "its reason is shown with it"
# The numbering must skip it, or the caller's action list stops lining up.
[[ "$out" == *"1  first"* && "$out" == *"2  second"* ]] \
  && ok "disabled entries consume no number" || no "disabled entries consume no number"
[[ "$out" == *"Choose 1-2,"* ]] && ok "the range counts only selectable options" \
                               || no "the range counts only selectable options"
sel="$(bash -c '. '"$ROOT"'/ui/render.sh
  printf "2\n" | ui_menu T "first" "!nope" "~why" "second"' 2>/dev/null)"
[[ "$sel" == "2" ]] && ok "choosing 2 selects the second enabled option" || no "selection returned [$sel]"

echo "== a stray keypress does not end the run =="
# Pressing Enter once too often after the pager used to quit the whole flow,
# because an empty line was treated the same as q.
out="$(bash -c '. '"$ROOT"'/ui/render.sh; printf "\n\n2\n" | ui_menu T a b c' 2>/dev/null)"
[[ "$out" == "2" ]] && ok "blank lines re-ask rather than quitting" || no "blank lines re-ask: got [$out]"
out="$(bash -c '. '"$ROOT"'/ui/render.sh; printf "zz\n9\n1\n" | ui_menu T a b c' 2>/dev/null)"
[[ "$out" == "1" ]] && ok "a typo and an out-of-range number both re-ask" || no "invalid input re-asks: got [$out]"
bash -c '. '"$ROOT"'/ui/render.sh; printf "q\n" | ui_menu T a b c' >/dev/null 2>&1 \
  && no "q still stops" || ok "q still stops"
bash -c '. '"$ROOT"'/ui/render.sh; printf "" | ui_menu T a b c' >/dev/null 2>&1 \
  && no "end of input still stops, so a pipe cannot loop" \
  || ok "end of input still stops, so a pipe cannot loop"

echo "== the layer stays presentation only =="
# If this fails, someone has put logic where an auditor will not look for it.
hits="$(grep -hE '(^|[^a-z_])(git|gh|curl|jq|rm|mv|cp) ' "$ROOT/ui/theme.sh" "$ROOT/ui/render.sh" \
        | sed 's/^[[:space:]]*//' | grep -v '^#' | grep -v 'tput' || true)"
if [[ -n "$hits" ]]; then
  no "ui/ runs no external commands"; printf '%s\n' "$hits" | sed 's/^/       /'
else ok "ui/ runs no external commands"; fi
grep -q 'INFECTED\|rmcej' "$ROOT/ui/theme.sh" && no "ui/theme.sh holds no indicators" || ok "ui/theme.sh holds no indicators"
for f in "$ROOT/ui/theme.sh" "$ROOT/ui/render.sh"; do
  [[ -x "$f" ]] && no "$(basename "$f") is not executable, it is sourced" \
                || ok "$(basename "$f") is not executable, it is sourced"
done

printf '\n  passed %s, failed %s\n' "$PASS" "$FAIL"
[[ "$FAIL" -eq 0 ]] || exit 1
