#!/usr/bin/env bash
# docs-serve.sh - preview the documentation site locally, with live reload.
#
#   ./ci/docs-serve.sh            serve on http://localhost:3000
#   ./ci/docs-serve.sh 4000       serve on another port
#
# Edit anything under docs-site/src/ or docs-site/theme/ and the page in your
# browser reloads by itself. That is the point: the design is meant to be
# adjusted by looking at it.
#
# This runs on your machine, NOT in the sandbox, and that is deliberate.
# The sandbox exists to contain a tool that moves files and rewrites git
# history. Rendering Markdown does none of that, and a read-only container
# mount would fight the edit-and-see-it loop this is for.

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${1:-3000}"

if ! command -v mdbook >/dev/null 2>&1; then
  cat >&2 <<'EOS'
docs-serve: mdbook is not installed.

  macOS    brew install mdbook
  Linux    cargo install mdbook --locked
           or a release binary from https://github.com/rust-lang/mdBook/releases

It is a single Rust binary with no runtime, which is why the site is built
with it and not with a Node toolchain.
EOS
  exit 3
fi

if lsof -nP -iTCP:"$PORT" -sTCP:LISTEN >/dev/null 2>&1; then
  echo "docs-serve: port $PORT is already in use. Try: ./ci/docs-serve.sh $((PORT+1))" >&2
  exit 3
fi

cat <<EOS

  Documentation site:  http://localhost:$PORT

  Live reload is on. Edit these and the browser follows:

    docs-site/src/**/*.md      the words
    docs-site/theme/custom.css the design: colours, type, spacing, callouts
    docs-site/src/SUMMARY.md   the sidebar and page order

  The palette and type choices are commented at the top of custom.css, and
  one number there is easy to get wrong: mdBook sets the root font size to
  62.5%, so 1rem is 10px, not 16px. Every length is written as px divided
  by 10.

  Ctrl-C to stop.

EOS

exec mdbook serve "$ROOT/docs-site" --port "$PORT" --open
