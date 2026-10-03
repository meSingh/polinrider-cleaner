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
#   shop       the payload on two branches, pushed by alice and bob
#   website    the payload on main, pushed by bob
#   old-site   the payload on a branch, and no push record left
#   infra      only the organization's own detection workflow: not a finding
#   blog       clean
FORGE="$HOME/demo-github"
PAD="$(printf '%280s' '')"
G=(git -c user.name=demo -c user.email=demo@localhost -c commit.gpgsign=false)
rm -rf "$FORGE" /tmp/demo-work /tmp/polinrider-evidence
mkdir -p "$FORGE/repos" "$FORGE/git/acme" "$FORGE/pushes/acme"
printf 'you\n' > "$FORGE/whoami"
printf 'acme\t5\nacme-labs\t0\n' > "$FORGE/orgs"
: > "$FORGE/repos/acme"; : > "$FORGE/repos/acme-labs"; : > "$FORGE/repos/you"

demo_repo() {   # demo_repo <name>, then demo_branch for each branch, then demo_done
  DEMO_WORK="/tmp/demo-work/$1"; DEMO_NAME="$1"
  mkdir -p "$DEMO_WORK"
  "${G[@]}" -C "$DEMO_WORK" init -q -b main
  printf '# %s\n' "$1" > "$DEMO_WORK/README.md"
  printf 'export const version = 1\n' > "$DEMO_WORK/index.js"
  "${G[@]}" -C "$DEMO_WORK" add -A; "${G[@]}" -C "$DEMO_WORK" commit -q -m "first commit"
}
demo_branch() { # demo_branch <branch>   (files are written by the caller afterwards)
  if [[ "$1" == "main" ]]; then "${G[@]}" -C "$DEMO_WORK" checkout -q main
  else "${G[@]}" -C "$DEMO_WORK" checkout -q -b "$1" main; fi
}
demo_commit() { "${G[@]}" -C "$DEMO_WORK" add -A; "${G[@]}" -C "$DEMO_WORK" commit -q -m "$1"; }
demo_done() {
  "${G[@]}" clone -q --bare "$DEMO_WORK" "$FORGE/git/acme/$DEMO_NAME.git"
  printf 'acme/%s\n' "$DEMO_NAME" >> "$FORGE/repos/acme"
}
infect() {      # the canonical shape: a payload appended behind padding, and a fake font
  printf 'export default { plugins: {} }\n%s%s\n' "$PAD" "$STRONG" > "$DEMO_WORK/postcss.config.mjs"
  mkdir -p "$DEMO_WORK/public/fonts"
  printf 'var _0x3f=function(){return 1};\n' > "$DEMO_WORK/public/fonts/inter-var.woff2"
}

demo_repo blog; demo_done

demo_repo shop
demo_branch release; infect; demo_commit "update config"
demo_branch staging; infect; demo_commit "update config"
demo_done
printf 'refs/heads/release\t4f2a91c0de\t9c01d7e2ab\talice\t2026-09-11T09:14:00Z\t0\nrefs/heads/staging\t4f2a91c0de\t71b3f0a9c4\tbob\t2026-09-12T16:40:00Z\t0\n' \
  > "$FORGE/pushes/acme/shop.tsv"

demo_repo website
demo_branch main; infect; demo_commit "update config"
demo_done
printf 'refs/heads/main\t1a2b3c4d5e\t6f7a8b9c0d\tbob\t2026-09-12T16:52:00Z\t0\n' > "$FORGE/pushes/acme/website.tsv"

demo_repo old-site
demo_branch legacy; infect; demo_commit "update config"
demo_done        # no push record: GitHub forgets after about ninety days

demo_repo infra
demo_branch main
mkdir -p "$DEMO_WORK/.github/workflows"
printf 'name: scan\non: push\njobs:\n  scan:\n    runs-on: ubuntu-latest\n    steps:\n      - run: grep -rF "%s" . && exit 1 || true\n' "$STRONG" \
  > "$DEMO_WORK/.github/workflows/polinrider-scan.yml"
demo_commit "scan every push"
demo_done
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
