#!/usr/bin/env bash
# beta.sh - install the 2.0 beta the way an end user would have it, and build
# something to use it on.
#
# Runs INSIDE the sandbox (polinrider-sandbox --beta). It builds the Rust
# binary, puts it with its indicators under $HOME/.local/share/polinrider,
# links it from $HOME/bin, and builds a sample: an infected project and a
# clean one under $HOME/code, and an infected login item, so the guided flow
# has something to find on "this computer" as well as in a folder.
#
# All of that is in the container's home directory and is gone when the
# container exits. Nothing here can reach the machine running Docker.

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT" || exit 3

[[ -f /.dockerenv || -f /run/.containerenv ]] || {
  echo "beta.sh installs a binary into \$HOME and plants sample malware indicators there." >&2
  echo "It is meant for the sandbox:  ./polinrider-sandbox --beta" >&2
  exit 3
}

printf '\n  Building the 2.0 beta. A minute the first time, seconds after that.\n'
commit="$(git rev-parse --short HEAD 2>/dev/null || true)"
if ! POLINRIDER_COMMIT="$commit" cargo build --locked --release >/tmp/beta-build.log 2>&1; then
  echo "  the build failed:" >&2; tail -20 /tmp/beta-build.log >&2; exit 3
fi

# Installed, not run from the build directory: reached through a link on the
# PATH, with the indicators beside the real file. This is the layout that once
# could not find its indicators at all, so it is the one worth trying.
SHARE="$HOME/.local/share/polinrider"
rm -rf "$SHARE"; mkdir -p "$SHARE" "$HOME/bin"
cp "${CARGO_TARGET_DIR:-$ROOT/target}/release/polinrider" "$SHARE/polinrider"
cp -R "$ROOT/ioc" "$SHARE/ioc"
ln -sf "$SHARE/polinrider" "$HOME/bin/polinrider"

STRONG="$(sed -e '/^#/d' -e '/^$/d' "$ROOT/ioc/strong.txt" | head -1)"

# The sample. $HOME/code is one of the directories the guided flow looks for.
DEMO_DIR="$HOME/code" "$ROOT/ci/demo.sh" --tree-only || exit 3
mkdir -p "$HOME/.config/systemd/user"
printf '[Unit]\nDescription=System update helper\n\n[Service]\nExecStart=/bin/sh -c "%s"\n' "$STRONG" \
  > "$HOME/.config/systemd/user/sysupdate-helper.service"

# A pretend GitHub, so the GitHub screens can be tried with no network and no
# sign-in: an organization called acme with real git repositories behind it.
# polinrider is pointed at it with --forge-state and never talks to GitHub.
#
#   shop       two branches attacked after they were clean: one force-pushed
#              by alice, one pushed to by bob. GitHub's record has both, so
#              both can be restored
#   website    main pushed to by bob, on record
#   old-site   the payload on a branch for longer than GitHub remembers: no
#              push record, so it cannot be restored
#   infra      only the organization's own detection workflow: not a finding
#   blog       clean
#
# The history is real: each attack is a push to the pretend GitHub, and the
# record holds the commit the branch pointed to before it, as GitHub's does.
FORGE="$HOME/demo-github"
PAD="$(printf '%280s' '')"
G=(git -c user.name=demo -c user.email=demo@localhost -c commit.gpgsign=false)
rm -rf "$FORGE" /tmp/demo-work /tmp/polinrider-evidence
mkdir -p "$FORGE/repos" "$FORGE/git/acme" "$FORGE/pushes/acme"
printf 'you\n' > "$FORGE/whoami"
printf 'acme\t5\nacme-labs\t0\n' > "$FORGE/orgs"
: > "$FORGE/repos/acme"; : > "$FORGE/repos/acme-labs"; : > "$FORGE/repos/you"

demo_repo() {   # demo_repo <name> [branch...]: a clean repository with those branches
  DEMO_WORK="/tmp/demo-work/$1"; DEMO_NAME="$1"; DEMO_BARE="$FORGE/git/acme/$1.git"
  mkdir -p "$DEMO_WORK"
  "${G[@]}" -C "$DEMO_WORK" init -q -b main
  printf '# %s\n\nHow to run it: npm install, then npm start.\n' "$1" > "$DEMO_WORK/README.md"
  printf 'export const version = 1\n' > "$DEMO_WORK/index.js"
  printf 'export default { plugins: {} }\n' > "$DEMO_WORK/postcss.config.mjs"
  "${G[@]}" -C "$DEMO_WORK" add -A; "${G[@]}" -C "$DEMO_WORK" commit -q -m "first commit"
  shift
  local b
  for b in "$@"; do
    "${G[@]}" -C "$DEMO_WORK" checkout -q -b "$b" main
    printf 'export const version = 2\n' > "$DEMO_WORK/index.js"
    "${G[@]}" -C "$DEMO_WORK" commit -q -am "work on $b"
  done
  "${G[@]}" -C "$DEMO_WORK" checkout -q main
  "${G[@]}" clone -q --bare "$DEMO_WORK" "$DEMO_BARE"
  # GitHub serves a commit by its ID whether or not a branch still reaches
  # it. A plain git repository has to be told to.
  "${G[@]}" -C "$DEMO_BARE" config uploadpack.allowAnySHA1InWant true
  printf 'acme/%s\n' "$DEMO_NAME" >> "$FORGE/repos/acme"
}
infect() {      # the canonical shape: a payload appended behind padding, and a fake font
  printf 'export default { plugins: {} }\n%s%s\n' "$PAD" "$STRONG" > "$DEMO_WORK/postcss.config.mjs"
  mkdir -p "$DEMO_WORK/public/fonts"
  printf 'var _0x3f=function(){return 1};\n' > "$DEMO_WORK/public/fonts/inter-var.woff2"
}
# demo_attack <branch> <how> [actor time]: push the payload to a branch.
# how is "force" (the newest commit is replaced) or "push" (one is added).
# With an actor and a time the push goes on the record; without, GitHub has
# forgotten it.
demo_attack() {
  local branch="$1" how="$2" actor="${3:-}" at="${4:-}" before head size=1
  "${G[@]}" -C "$DEMO_WORK" checkout -q "$branch"
  infect
  "${G[@]}" -C "$DEMO_WORK" add -A
  if [[ "$how" == "force" ]]; then size=0; "${G[@]}" -C "$DEMO_WORK" commit -q --amend -m "update config"
  else "${G[@]}" -C "$DEMO_WORK" commit -q -m "update config"; fi
  before="$("${G[@]}" -C "$DEMO_BARE" rev-parse "refs/heads/$branch")"
  "${G[@]}" -C "$DEMO_WORK" push -q --force "$DEMO_BARE" "$branch:$branch"
  head="$("${G[@]}" -C "$DEMO_BARE" rev-parse "refs/heads/$branch")"
  [[ -n "$actor" ]] && printf 'refs/heads/%s\t%s\t%s\t%s\t%s\t%s\n' "$branch" "$before" "$head" "$actor" "$at" "$size" \
    >> "$FORGE/pushes/acme/$DEMO_NAME.tsv"
  return 0
}

demo_repo blog

demo_repo shop release staging
demo_attack release force alice 2026-09-11T09:14:00Z
demo_attack staging push  bob   2026-09-12T16:40:00Z

demo_repo website
demo_attack main push bob 2026-09-12T16:52:00Z

demo_repo old-site legacy
demo_attack legacy push       # no push record: GitHub forgets after about ninety days

demo_repo infra
"${G[@]}" -C "$DEMO_WORK" checkout -q main
mkdir -p "$DEMO_WORK/.github/workflows"
printf 'name: scan\non: push\njobs:\n  scan:\n    runs-on: ubuntu-latest\n    steps:\n      - run: grep -rF "%s" . && exit 1 || true\n' "$STRONG" \
  > "$DEMO_WORK/.github/workflows/polinrider-scan.yml"
"${G[@]}" -C "$DEMO_WORK" add -A; "${G[@]}" -C "$DEMO_WORK" commit -q -m "scan every push"
"${G[@]}" -C "$DEMO_WORK" push -q "$DEMO_BARE" main:main
rm -rf /tmp/demo-work

# Shown whole and not cut down to a line with head: the first version of this
# script did that, the pipe closed while the binary was still writing, and
# the first thing the beta ever printed for its maintainer was a Rust panic.
printf '\n  The 2.0 beta is installed in this container. This is polinrider --version:\n\n'
"$HOME/bin/polinrider" --version

cat <<TXT

  A sample is waiting for it. None of it is live malware: each file carries
  one indicator string from ioc/, which is what the scanner matches.

    ~/code/shop    a project infected five ways
    ~/code/blog    a clean project
    ~/.config/systemd/user/sysupdate-helper.service    an infected login item

  Start here. It asks what to check, and changes nothing unless you type yes:

    polinrider

  Or drive it yourself:

    polinrider --version              which build this is, and how many indicators it has
    polinrider check ~/code           read-only check of this container and that folder
    polinrider clean ~/code/shop      what it would strip and move. Add --apply to do it
    ls ~/polinrider-quarantine-*      where the originals went, after an --apply or a yes
    ./ci/beta.sh                      put the sample back the way it was

  The GitHub screens, on a pretend organization called acme. No network,
  no sign-in, and GitHub is never contacted. Choose organization, then acme:

    polinrider guide --forge-state ~/demo-github

  Exit codes: 0 clean, 1 needs a look, 2 confirmed, 3 could not run.  echo \$?
TXT
[[ -d /scan ]] && cat <<'TXT'

  Your own directory is at /scan, mounted read-only. The beta can read it and
  cannot change it:

    polinrider check --fs-only /scan
TXT
cat <<'TXT'

  You are in a container. Nothing you do here can touch the machine it runs
  on, and all of this is gone when you type exit.

TXT
