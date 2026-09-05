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

BEFORE="$(find "$TMP" -type f -exec ls -l {} + | awk '{print $5, $NF}' | sort)"

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
  Darwin) EXPECT="check-macos.sh" ;;
  *)      EXPECT="check-linux.sh" ;;
esac
OUT="$("$ROOT/polinrider.sh" --path "$TMP/dirty" 2>&1)"
if printf '%s\n' "$OUT" | grep -q "$EXPECT"; then
  pass "routes to the right local tool for this OS ($EXPECT)"
else fail "did not name $EXPECT in its guidance"; fi

# --- it changed nothing ------------------------------------------------------
AFTER="$(find "$TMP" -type f -exec ls -l {} + | awk '{print $5, $NF}' | sort)"
if [[ "$BEFORE" == "$AFTER" ]]; then pass "changed nothing on disk"; else fail "modified the scanned tree"; fi

echo
if [[ $FAILED -eq 0 ]]; then echo "ENTRY POINT SELFTEST PASSED"; exit 0; fi
echo "ENTRY POINT SELFTEST FAILED"; exit 1
