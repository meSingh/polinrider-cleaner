#!/usr/bin/env bash
# selftest-walk.sh - offline test of the single pruned walk and of --resume.
#
# Builds a tree where the same indicators sit both inside and outside
# node_modules and .git, then asserts: the file list never enters a pruned
# directory, findings outside are reported and findings inside are not, hooks
# are still read from .git, checkpoints and counters are written, and a
# second run with --resume reuses the list, skips finished checks and keeps
# the counts. No network, nothing outside a temp directory.
#
# Usage: ./selftest-walk.sh

set -uo pipefail
SDIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SDIR/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
FAILED=0
pass() { printf '  ok    %s\n' "$*"; }
fail() { printf '  FAIL  %s\n' "$*"; FAILED=1; }

case "$(uname -s)" in
  Darwin) CHECK="$ROOT/machine-cleanup/check-macos.sh" ;;
  *)      CHECK="$ROOT/machine-cleanup/check-linux.sh" ;;
esac
STRONG="$(sed -e '/^#/d' -e '/^$/d' "$ROOT/ioc/strong.txt" | head -1)"

# --- fixture ----------------------------------------------------------------
H="$TMP/home"; C="$TMP/code/proj"
mkdir -p "$H" "$C/public" "$C/.vscode" "$C/.git/hooks" "$C/.git/objects/aa" \
         "$C/node_modules/evil/public" "$C/node_modules/evil/.vscode"
printf 'export default { plugins: [] }\n' > "$C/postcss.config.mjs"
printf 'export default {}\n%s\n' "$STRONG" > "$C/node_modules/evil/postcss.config.mjs"   # must not be reported
printf 'not a font at all, this is javascript' > "$C/public/fake.woff2"                    # must be reported
printf 'not a font at all, this is javascript' > "$C/node_modules/evil/public/fake.woff2" # must not
printf '{"tasks":[{"runOptions":{"runOn":"folderOpen"}}]}\n' > "$C/.vscode/tasks.json"    # review
printf '{"tasks":[{"runOptions":{"runOn":"folderOpen"}}]}\n' > "$C/node_modules/evil/.vscode/tasks.json"
printf '#!/bin/sh\necho hi\n' > "$C/.git/hooks/pre-commit"; chmod +x "$C/.git/hooks/pre-commit"  # review, via gitdirs
printf 'blob' > "$C/.git/objects/aa/bb"
# bulk, so the prune has something measurable to skip
for i in $(seq 1 400); do : > "$C/node_modules/evil/f$i.js"; done

STATE="$TMP/state"; REPORT="$TMP/report.txt"
echo "first run"
HOME="$H" "$CHECK" --state "$STATE" --report "$REPORT" "$TMP/code" >/dev/null 2>&1
RC=$?

# --- the walk ----------------------------------------------------------------
[[ -s "$STATE/manifest.txt" ]] && pass "manifest written" || fail "no manifest"
grep -q '/node_modules/' "$STATE/manifest.txt" && fail "manifest entered node_modules" || pass "node_modules pruned from the manifest"
grep -q '/\.git/' "$STATE/manifest.txt" && fail "manifest entered .git" || pass ".git pruned from the manifest"
grep -q "$C/.git\$" "$STATE/gitdirs.txt" && pass ".git directory listed for hooks" || fail ".git directory not listed"
n=$(grep -c . "$STATE/manifest.txt"); [[ $n -lt 20 ]] && pass "manifest holds $n files, not the 400 under node_modules" || fail "manifest has $n entries"

# --- findings ----------------------------------------------------------------
grep -q "font file is not a font.*$C/public/fake.woff2" "$REPORT" && pass "fake font outside node_modules reported" || fail "fake font not reported"
grep -q "node_modules/evil" "$REPORT" && fail "something under node_modules was reported" || pass "nothing under node_modules reported"
grep -q "tasks.json runs on folder open.*$C/.vscode/tasks.json" "$REPORT" && pass "folderOpen task reported" || fail "folderOpen task not reported"
grep -q "active git hook.*pre-commit" "$REPORT" && pass "git hook read via the gitdirs list" || fail "git hook not read"
[[ $RC -eq 2 ]] && pass "exit 2, the fake font is a confirmed hit" || fail "exit code $RC, expected 2"

# --- checkpoints -------------------------------------------------------------
grep -qx "Font files" "$STATE/done" && pass "checkpoint recorded" || fail "no checkpoint for Font files"
grep -q '^HITS=1' "$STATE/counters" && pass "counters persisted (HITS=1)" || { fail "counters: $(cat "$STATE/counters" 2>/dev/null)"; }
[[ "$(cat "$STATE/report-path")" == "$REPORT" ]] && pass "report path recorded" || fail "report path not recorded"
grep -qx "$TMP/code" "$STATE/roots" && pass "roots recorded" || fail "roots not recorded"

# --- resume ------------------------------------------------------------------
# Pretend the run died after "Font files": drop the later checkpoints, then
# add a new file the resumed run must not see, because it reuses the list.
awk '/^Font files$/{print; exit} {print}' "$STATE/done" > "$STATE/done.tmp" && mv "$STATE/done.tmp" "$STATE/done"
printf 'export default {}\n' > "$C/vite.config.js"
echo "resumed run"
HOME="$H" "$CHECK" --resume "$STATE" >/dev/null 2>&1
RC2=$?
grep -q "reusing the file list from the previous run" "$REPORT" && pass "file list reused" || fail "walked again on resume"
grep -q "== Font files: done in the previous run, skipped ==" "$REPORT" && pass "finished check skipped" || fail "finished check re-ran"
grep -q "== Credential surface on this machine ==" "$REPORT" && pass "unfinished check ran" || fail "unfinished check did not run"
grep -q "(resumed)" "$REPORT" && pass "report marked as resumed, same file" || fail "resume did not continue the same report"
grep -q "vite.config.js" "$STATE/manifest.txt" && fail "resume re-walked (new file present)" || pass "new file not seen, as documented"
c=$(grep -c "font file is not a font" "$REPORT"); [[ $c -eq 1 ]] && pass "no duplicate finding after resume" || fail "finding reported $c times"
[[ $RC2 -eq 2 ]] && pass "resumed verdict keeps the earlier hit (exit 2)" || fail "resumed exit code $RC2, expected 2"

# --- background does not fork when already detached ---------------------------
out=$(HOME="$H" PRC_BG=1 "$CHECK" --background --state "$TMP/state2" --report "$TMP/r2.txt" "$TMP/code" 2>&1)
printf '%s' "$out" | grep -q "running in the background" && fail "forked despite PRC_BG" || pass "PRC_BG=1 runs inline (the detached copy does not fork again)"

echo
[[ $FAILED -eq 0 ]] && { echo "selftest-walk: all passed"; exit 0; } || { echo "selftest-walk: FAILED"; exit 1; }
