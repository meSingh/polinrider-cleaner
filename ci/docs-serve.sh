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

if ! command -v npm >/dev/null 2>&1; then
  echo "docs-serve: npm is not installed. The site is built with Astro + Starlight." >&2
  echo "  macOS    brew install node" >&2
  exit 3
fi

if [[ ! -d "$ROOT/docs-site/node_modules" ]]; then
  echo "docs-serve: installing dependencies, once"
  npm ci --prefix "$ROOT/docs-site" || { echo "docs-serve: npm ci failed" >&2; exit 3; }
fi

if lsof -nP -iTCP:"$PORT" -sTCP:LISTEN >/dev/null 2>&1; then
  echo "docs-serve: port $PORT is already in use. Try: ./ci/docs-serve.sh $((PORT+1))" >&2
  exit 3
fi

cat <<EOS

  Documentation site:  http://localhost:$PORT

  Live reload is on. Edit these and the browser follows:

    docs-site/src/content/docs/**/*.md   the words
    docs-site/src/styles/custom.css      small style additions
    docs-site/astro.config.mjs           the sidebar, title and theme options

  The look comes from the Lucode theme, a shadcn/ui-styled Starlight theme.
  Most appearance changes belong in astro.config.mjs or in the theme's own
  options rather than in custom.css.

  Ctrl-C to stop.

EOS

exec npm run dev --prefix "$ROOT/docs-site" -- --port "$PORT" --open
