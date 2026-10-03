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

# The sample. $HOME/code is one of the directories the guided flow looks for.
DEMO_DIR="$HOME/code" "$ROOT/ci/demo.sh" --tree-only || exit 3
STRONG="$(sed -e '/^#/d' -e '/^$/d' "$ROOT/ioc/strong.txt" | head -1)"
mkdir -p "$HOME/.config/systemd/user"
printf '[Unit]\nDescription=System update helper\n\n[Service]\nExecStart=/bin/sh -c "%s"\n' "$STRONG" \
  > "$HOME/.config/systemd/user/sysupdate-helper.service"

cat <<TXT

  The 2.0 beta is installed in this container:  $("$HOME/bin/polinrider" --version | head -1)

  A sample is waiting for it. None of it is live malware: each file carries
  one indicator string from ioc/, which is what the scanner matches.

    ~/code/shop    a project infected five ways
    ~/code/blog    a clean project
    ~/.config/systemd/user/sysupdate-helper.service    an infected login item

  Start here. It asks what to check, and changes nothing unless you type yes:

    polinrider

  Or drive it yourself:

    polinrider --version              which build this is, and where its indicators are
    polinrider check ~/code           read-only check of this container and that folder
    polinrider clean ~/code/shop      what it would strip and move. Add --apply to do it
    ls ~/polinrider-quarantine-*      where the originals went, after an --apply or a yes
    ./ci/beta.sh                      put the sample back the way it was

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
