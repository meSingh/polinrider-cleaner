#!/usr/bin/env bash
# selftest-ui.sh - the presentation layer, offline.
#
# Two things are being protected here. That the output degrades correctly, so a
# CI log or a pipe never fills with escape codes. And that ui/ stays
# presentation only, so auditing this tool never requires reading it.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
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
[[ "$out" != *"┌"* ]] && ok "a non-UTF-8 locale gets the ASCII wordmark" || no "a non-UTF-8 locale gets the ASCII wordmark"

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

echo "== ui_stream indents subprocess output =="
out="$(printf 'remote: Counting\n' | ( . "$ROOT/ui/render.sh"; ui_stream ))"
[[ "$out" == "      "* ]] && ok "subprocess output is indented under its step" || no "subprocess output is indented"

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
