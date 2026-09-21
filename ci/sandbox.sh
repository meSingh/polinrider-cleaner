#!/usr/bin/env bash
# sandbox.sh - run anything in this repository inside a container.
#
# This tool quarantines files, rewrites git history and walks $HOME. Running
# its tests directly on a workstation is one bad path expansion away from
# moving somebody's real files, so development and testing happen in here.
#
#   ./ci/sandbox.sh                       interactive shell in the sandbox
#   ./ci/sandbox.sh --all                 lint, every self-test, the corpus
#   ./ci/sandbox.sh ./conformance/run.sh  one command
#   ./ci/sandbox.sh --demo                build a sample infected project and scan it
#   ./ci/sandbox.sh --build               rebuild the image and stop
#   ./ci/sandbox.sh --net <cmd>           allow networking (cargo fetch)
#
# What the sandbox actually guarantees:
#
#   - $HOME is the container's, so a run that walks or writes $HOME cannot
#     reach yours.
#   - The repository is mounted READ-ONLY at /work. A test cannot modify the
#     source it is testing, and cannot corrupt the git history.
#   - Build output goes to a named volume under $HOME/build, so cargo works
#     against a read-only checkout. Deliberately not $HOME itself: a volume
#     there masks the image's own git config and locale setup.
#   - Networking is OFF unless --net is passed. A scan has no business making
#     a connection, and this is how that stops being a promise.
#   - Non-root, no added capabilities, no privilege escalation.
#
# What it does NOT give you: platform coverage. This is Linux with bash 5.
# macOS paths and bash 3.2 behaviour are only exercised by CI's macos runner
# and by running on a real Mac. Isolation and coverage are different problems.

set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"

IMAGE="polinrider-sandbox:dev"
VOLUME="polinrider-sandbox-build"
NET="none"

command -v docker >/dev/null 2>&1 || {
  echo "sandbox: docker is not installed." >&2
  echo "  macOS:  brew install --cask docker   then start Docker Desktop" >&2
  echo "  Linux:  your distribution's docker.io or podman package" >&2
  exit 3
}
docker info >/dev/null 2>&1 || {
  echo "sandbox: the docker daemon is not responding. Start Docker Desktop, or the docker service." >&2
  exit 3
}

build_image() {
  # --quiet keeps a clean build silent; a failing build still prints.
  if ! docker build --quiet \
        --build-arg "UID=$(id -u)" --build-arg "GID=$(id -g)" \
        -t "$IMAGE" -f "$ROOT/.devcontainer/Dockerfile" "$ROOT" >/dev/null; then
    echo "sandbox: image build failed. Re-running with output:" >&2
    docker build --build-arg "UID=$(id -u)" --build-arg "GID=$(id -g)" \
      -t "$IMAGE" -f "$ROOT/.devcontainer/Dockerfile" "$ROOT"
    exit 3
  fi
}

FORCE_BUILD=0
case "${1:-}" in
  --build) FORCE_BUILD=1; shift ;;
  -h|--help) sed -n '2,30p' "$0"; exit 0 ;;
esac
[[ "${1:-}" == "--net" ]] && { NET="bridge"; shift; }

if [[ $FORCE_BUILD -eq 1 ]] || ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
  echo "sandbox: building $IMAGE (first run takes a few minutes)"
  build_image
  echo "sandbox: image ready"
  [[ $FORCE_BUILD -eq 1 && $# -eq 0 ]] && exit 0
fi

docker volume inspect "$VOLUME" >/dev/null 2>&1 || docker volume create "$VOLUME" >/dev/null

run() {
  # -t only when stdin is a terminal: with it in a pipeline docker errors out.
  local tty=(); [[ -t 0 ]] && tty=(-t)
  docker run --rm -i "${tty[@]+"${tty[@]}"}" \
    --network "$NET" \
    --user "$(id -u):$(id -g)" \
    --cap-drop ALL \
    --security-opt no-new-privileges \
    --pids-limit 512 \
    --mount "type=bind,source=$ROOT,target=/work,readonly" \
    --mount "type=volume,source=$VOLUME,target=/home/dev/build" \
    --tmpfs /tmp:rw,exec,size=2g \
    -w /work \
    "$IMAGE" "$@"
}

if [[ $# -eq 0 ]]; then
  echo "sandbox: interactive shell. Repository is read-only at /work, network ${NET}."
  run bash
  exit $?
fi

# A sample workspace to point the tool at, built inside the container and left
# there. Ends in a shell so the scan can be repeated with different flags.
if [[ "$1" == "--demo" ]]; then
  run bash -c './ci/demo.sh; echo "  Dropping you into the sandbox. Type exit when done."; echo; exec bash'
  exit $?
fi

if [[ "$1" == "--all" ]]; then
  run bash -c '
    set -uo pipefail
    fail=0
    echo "== syntax =="
    bash -n polinrider.sh lib/*.sh ci/*.sh conformance/*.sh github-*/*.sh machine-cleanup/*.sh || fail=1
    echo "== shellcheck =="
    shellcheck --severity=warning --external-sources \
      polinrider.sh lib/*.sh ci/*.sh conformance/*.sh github-*/*.sh machine-cleanup/*.sh || fail=1
    echo "== self-tests =="
    for t in ci/selftest*.sh; do
      if ./"$t" >/dev/null 2>&1; then printf "  pass  %s\n" "$(basename "$t")"
      else printf "  FAIL  %s\n" "$(basename "$t")"; fail=1; fi
    done
    echo "== conformance =="
    ./conformance/run.sh || fail=1
    exit $fail
  '
  exit $?
fi

run "$@"
