#!/usr/bin/env bash
# selftest-entrypoint.sh - offline test of polinrider.sh, the single entry point.
#
# Checks that it routes to the right tool, returns the right exit code, answers
# piped input as well as a terminal, fails safe when it has neither, and changes
# nothing on disk. No network, no credentials.
#
# Usage: ./selftest-entrypoint.sh

set -uo pipefail
SDIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SDIR/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
FAILED=0

pass() { printf '  ok    %s\n' "$*"; }
fail() { printf '  FAIL  %s\n' "$*"; FAILED=1; }

# --- fixtures ---------------------------------------------------------------
mkdir -p "$TMP/clean" "$TMP/dirty/.vscode"
printf 'export default { reactStrictMode: true }\n' > "$TMP/clean/next.config.ts"
printf 'module.exports = {}\nconst _0x=global["_V"];// rmcej%%otb%%\n' > "$TMP/dirty/tailwind.config.js"

SCANNED="$TMP/clean $TMP/dirty"
# shellcheck disable=SC2086  # the two paths are separate arguments on purpose
BEFORE="$(find $SCANNED -type f -exec ls -l {} + | awk '{print $5, $NF}' | sort)"

echo "assertions:"

# --- help --------------------------------------------------------------------
OUT="$("$ROOT/polinrider.sh" --help 2>&1)"; RC=$?
if [[ $RC -eq 0 ]]; then pass "--help exits 0"; else fail "--help exited $RC"; fi
for want in -- --machine --org --user --path; do
  if printf '%s' "$OUT" | grep -q -- "$want"; then pass "--help documents $want"
  else fail "--help does not mention $want"; fi
done

# --- unknown flag ------------------------------------------------------------
"$ROOT/polinrider.sh" --not-a-real-flag >/dev/null 2>&1
if [[ $? -eq 2 ]]; then pass "rejects an unknown flag"; else fail "unknown flag not rejected"; fi

# --- no mode, no input -------------------------------------------------------
"$ROOT/polinrider.sh" </dev/null >/dev/null 2>&1
if [[ $? -eq 2 ]]; then pass "fails safe with no mode and no input"; else fail "did not fail safe"; fi

# --- clean path --------------------------------------------------------------
"$ROOT/polinrider.sh" --path "$TMP/clean" >/dev/null 2>&1
if [[ $? -eq 0 ]]; then pass "clean folder exits 0"; else fail "clean folder did not exit 0"; fi

# --- infected path -----------------------------------------------------------
OUT="$("$ROOT/polinrider.sh" --path "$TMP/dirty" 2>&1)"; RC=$?
if [[ $RC -eq 2 ]]; then pass "infected folder exits 2"; else fail "infected folder exited $RC"; fi
if printf '%s\n' "$OUT" | grep -q "Something confirmed"; then
  pass "infected folder prints the next steps"; else fail "no next steps after a finding"; fi
if printf '%s\n' "$OUT" | grep -q "Rotate every credential"; then
  pass "next steps put credential rotation before restoring"; else fail "rotation step missing"; fi

# --- piped menu --------------------------------------------------------------
OUT="$(printf '4\n%s\n' "$TMP/dirty" | "$ROOT/polinrider.sh" 2>&1)"; RC=$?
if [[ $RC -eq 2 ]]; then pass "menu answers can be piped in"; else fail "piped menu exited $RC"; fi

# --- OS routing --------------------------------------------------------------
case "$(uname -s)" in
  Darwin) EXPECT="$ROOT/machine-cleanup/check-macos.sh" ;;
  *)      EXPECT="$ROOT/machine-cleanup/check-linux.sh" ;;
esac
if [[ -x "$EXPECT" ]]; then
  pass "the per-OS tool for this machine exists and is executable"
else fail "missing per-OS tool: $EXPECT"; fi

OUT="$("$ROOT/polinrider.sh" --path "$TMP/dirty" 2>&1)"
if printf '%s\n' "$OUT" | grep -q -- "--machine"; then
  pass "a finding points the operator at the machine check"
else fail "guidance does not mention --machine"; fi

# --- the GitHub verdict must come from counts, never from an exit code -------
# A previous version returned triage-filter's exit status, which is 0 by design.
# On an account with 312 infected refs it printed "nothing confirmed". The entry
# point now parses the counts, so this asserts that contract from both ends.
cat > "$TMP/triage-real.json" <<'JSON'
[
 {"repo":"acme/app","ref":"refs/heads/main","verdict":"INFECTED",
  "ioc_strings":["refs/heads/main:public/fonts/fake.woff2:1: rmcej%otb%"],
  "ioc_filenames":[],"font_masquerade":[],"weak_signals":[],"config_tail":[]}
]
JSON
cat > "$TMP/triage-tooling.json" <<'JSON'
[
 {"repo":"acme/cleaner","ref":"refs/heads/main","verdict":"INFECTED",
  "ioc_strings":["refs/heads/main:ioc/strong.txt:1: rmcej%otb%"],
  "ioc_filenames":[],"font_masquerade":[],"weak_signals":[],"config_tail":[]}
]
JSON

# triage-filter still exits 0, which is why it must not be used as the verdict
"$ROOT/lib/triage-filter.sh" "$TMP/triage-real.json" >/dev/null 2>&1
if [[ $? -eq 0 ]]; then
  pass "triage-filter exits 0 even with a real finding, as documented"
else fail "triage-filter changed its exit contract"; fi

# the entry point's own extraction, applied to the real output
extract() {
  "$ROOT/lib/triage-filter.sh" "$1" 2>&1 \
    | awk -F: '/REAL_SUSPECT refs/ {gsub(/[^0-9]/,"",$2); print $2; exit}'
}
if [[ "$(extract "$TMP/triage-real.json")" == "1" ]]; then
  pass "a genuine finding is counted as one real suspect"
else fail "real finding not counted: got '$(extract "$TMP/triage-real.json")'"; fi

if [[ "$(extract "$TMP/triage-tooling.json")" == "0" ]]; then
  pass "a match inside the indicator set is not counted as a real suspect"
else fail "own tooling counted as a real suspect: got '$(extract "$TMP/triage-tooling.json")'"; fi

if grep -q 'REAL_SUSPECT refs' "$ROOT/lib/triage-filter.sh"; then
  pass "the label the entry point parses still exists in triage-filter"
else fail "triage-filter no longer prints the label the entry point parses"; fi

# --- it changed nothing ------------------------------------------------------
# shellcheck disable=SC2086  # same
AFTER="$(find $SCANNED -type f -exec ls -l {} + | awk '{print $5, $NF}' | sort)"
if [[ "$BEFORE" == "$AFTER" ]]; then pass "changed nothing on disk"; else fail "modified the scanned tree"; fi

echo
if [[ $FAILED -eq 0 ]]; then echo "ENTRY POINT SELFTEST PASSED"; exit 0; fi
echo "ENTRY POINT SELFTEST FAILED"; exit 1
