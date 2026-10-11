#!/usr/bin/env bash
# demo.sh - build a realistically infected project to scan.
#
# Runs INSIDE the sandbox (polinrider-sandbox --beta calls it). It needs somewhere writable
# and somewhere safe, and the container is both.
#
#   ./ci/demo.sh              build the sample and say how to scan it
#   ./ci/demo.sh --tree-only  build the sample and stop
#
# The fixture carries no payload. It writes a real indicator pulled from ioc/
# at build time, the same way the conformance corpus does, so this file stays
# clean in the repository and the demo cannot drift from the indicator set.

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO="${DEMO_DIR:-/tmp/polinrider-demo}"

STRONG="$(sed -e '/^#/d' -e '/^$/d' "$ROOT/ioc/strong.txt" | head -1)"
BADPKG="$(sed -e '/^#/d' -e '/^$/d' "$ROOT/ioc/bad-packages.txt" | head -1)"
PAD="$(printf '%280s' '')"

rm -rf "$DEMO"; mkdir -p "$DEMO/shop/public" "$DEMO/shop/src" "$DEMO/shop/.vscode" "$DEMO/blog"

# 1. the canonical infection: payload appended past the module end, hidden
#    behind padding so the line looks untouched in an editor
printf 'export default {\n  plugins: { tailwindcss: {}, autoprefixer: {} },\n}\n%s%s\n' \
  "$PAD" "$STRONG" > "$DEMO/shop/postcss.config.mjs"

# 2. a "font" whose bytes are JavaScript
printf 'var _0x3f=function(){return fetch("https://example.invalid")};\n' \
  > "$DEMO/shop/public/inter-var.woff2"

# 3. a real font, which must NOT be flagged
printf 'wOF2 and then bytes that are not checked further\n' \
  > "$DEMO/shop/public/logo.woff2"

# 4. an editor task that runs the moment the folder is opened
cat > "$DEMO/shop/.vscode/tasks.json" <<JSON
{
  "version": "2.0.0",
  "tasks": [
    { "label": "postinstall", "type": "shell", "command": "node ./scripts/setup.js",
      "runOptions": { "runOn": "folderOpen" } }
  ]
}
JSON

# 5. a campaign package named in the manifest
printf '{\n  "name": "shop",\n  "dependencies": { "react": "^18.0.0", "%s": "^1.0.0" }\n}\n' \
  "$BADPKG" > "$DEMO/shop/package.json"

# 6. ordinary files, and a second project that is completely clean
printf 'export const cart = []\n' > "$DEMO/shop/src/cart.js"
printf 'export default { plugins: {} }\n' > "$DEMO/blog/postcss.config.mjs"
printf 'wOF2 clean font\n' > "$DEMO/blog/logo.woff2"

# The sample alone, for callers that scan it themselves: ci/beta.sh and the
# beta build workflow.
[[ "${1:-}" == "--tree-only" ]] && exit 0

cat <<EOF

  A sample workspace is at $DEMO

    shop/   deliberately infected, five different ways
    blog/   completely clean, so you can see what a pass looks like

  Nothing here is live malware. It carries one real indicator string, pulled
  from ioc/ when this ran, which is what makes the scanner match it.

  This script used to scan it with the shell machine check. That check is the
  polinrider binary now. To build the binary, install it in the container and
  scan this sample with it:

    ./polinrider-sandbox --beta

EOF
