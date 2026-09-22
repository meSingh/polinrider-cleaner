#!/usr/bin/env bash
# run.sh - the conformance corpus. The specification 2.0.0 is measured against.
#
# Each case in cases/ declares a filesystem to build, an implementation to run
# against it, and exactly what must come back. Every implementation has to
# produce the same answers, so the Rust port is finished when it passes this
# suite rather than when it compiles.
#
# Usage:
#   ./conformance/run.sh                   # the shell implementation
#   ./conformance/run.sh --impl rust       # the Rust binary
#   ./conformance/run.sh --case fake-font  # one case, with full output on failure
#   ./conformance/run.sh --diff            # print the run output for every case
#
# Exit 0 all passed, 1 a case failed, 3 the harness could not run.
#
# NOTE ON FIXTURES. No case file contains a payload. Cases write {{STRONG}},
# {{WEAK}} or {{BADPKG}} and this runner substitutes a real entry from ioc/ at
# build time. That keeps working malware strings out of the repository, and
# means the corpus cannot drift away from the indicator set it is testing.

set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"

IMPL="shell"; ONLY=""; SHOW_DIFF=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --impl) IMPL="$2"; shift 2 ;;
    --case) ONLY="$2"; shift 2 ;;
    --diff) SHOW_DIFF=1; shift ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 3 ;;
  esac
done

command -v jq >/dev/null 2>&1 || { echo "conformance needs jq" >&2; exit 3; }

# --- the indicators the fixtures are built from ----------------------------
pick() { sed -e '/^#/d' -e '/^$/d' "$ROOT/ioc/$1" | sed -n "${2:-1}p"; }
STRONG="$(pick strong.txt 1)"
WEAK="$(pick weak.txt 1)"
BADPKG="$(pick bad-packages.txt 1)"
[[ -n "$STRONG" && -n "$BADPKG" ]] || { echo "indicator set is empty or unreadable" >&2; exit 3; }
# The campaign hides the payload behind roughly 280 spaces of padding.
PAD="$(printf '%280s' '')"

# --- implementation adapters ------------------------------------------------
# Each takes: <report> <state> <quarantine|""> <root...>
# and prints the run output. The exit code is the implementation's.
impl_shell() {
  local report="$1" state="$2" qdir="$3"; shift 3
  local tool; case "$(uname -s)" in
    Darwin) tool="$ROOT/machine-cleanup/check-macos.sh" ;;
    *)      tool="$ROOT/machine-cleanup/check-linux.sh" ;;
  esac
  if [[ -n "$qdir" ]]; then
    HOME="$FAKE_HOME" "$tool" --fs-only --report "$report" --state "$state" --apply --quarantine "$qdir" "$@" 2>&1
  else
    HOME="$FAKE_HOME" "$tool" --fs-only --report "$report" --state "$state" "$@" 2>&1
  fi
}

impl_rust() {
  local report="$1" state="$2" qdir="$3"; shift 3
  # CARGO_TARGET_DIR moves the build output, which the sandbox does because
  # the repository is mounted read-only. Hardcoding $ROOT/target meant every
  # case failed in the sandbox while passing on the host.
  local bin="${CARGO_TARGET_DIR:-$ROOT/target}/release/polinrider"
  [[ -x "$bin" ]] || {
    echo "conformance: no binary at $bin. Build it: cargo build --release" >&2
    return 3
  }
  if [[ -n "$qdir" ]]; then
    HOME="$FAKE_HOME" "$bin" check --fs-only --report "$report" --state "$state" --apply --quarantine "$qdir" "$@" 2>&1
  else
    HOME="$FAKE_HOME" "$bin" check --fs-only --report "$report" --state "$state" "$@" 2>&1
  fi
}

case "$IMPL" in
  shell|rust) ;;
  *) echo "unknown implementation: $IMPL (shell|rust)" >&2; exit 3 ;;
esac

# --- helpers ----------------------------------------------------------------
# A content hash of every file under a directory, so "did the run change what
# it was reading" is a question with an exact answer.
snapshot() {
  ( cd "$1" 2>/dev/null && find . -type f -print0 2>/dev/null \
      | LC_ALL=C sort -z \
      | xargs -0 shasum -a 256 2>/dev/null ) || true
}

subst() { printf '%s' "$1" \
  | sed -e "s|{{STRONG}}|$STRONG|g" -e "s|{{WEAK}}|$WEAK|g" \
        -e "s|{{BADPKG}}|$BADPKG|g" -e "s|{{PAD}}|$PAD|g"; }

PASS=0; FAIL=0; FAILED_CASES=()

run_case() {
  local cf="$1" name; name="$(basename "$cf" .json)"
  [[ -n "$ONLY" && "$ONLY" != "$name" ]] && return 0

  local why; why="$(jq -r '.why' "$cf")"
  local tmp; tmp="$(mktemp -d)"
  FAKE_HOME="$tmp/home"; mkdir -p "$FAKE_HOME" "$tmp/tree" "$tmp/out"

  # build the fixture
  local path content
  while IFS= read -r path; do
    content="$(jq -r --arg p "$path" '.files[$p]' "$cf")"
    mkdir -p "$tmp/tree/$(dirname "$path")"
    subst "$content" > "$tmp/tree/$path"
  done < <(jq -r '.files | keys[]' "$cf")

  # roots, relative to the fixture tree
  local roots=(); local r
  while IFS= read -r r; do roots+=("$tmp/tree/$r"); done < <(jq -r '.roots[]' "$cf")

  local apply qdir=""
  apply="$(jq -r '.apply // false' "$cf")"
  [[ "$apply" == "true" ]] && qdir="$tmp/out/quarantine"

  local before after out rc
  before="$(snapshot "$tmp/tree")"
  out="$("impl_$IMPL" "$tmp/out/report.txt" "$tmp/out/state" "$qdir" "${roots[@]}")"
  rc=$?
  after="$(snapshot "$tmp/tree")"

  # --- assertions ----------------------------------------------------------
  local errs=()

  local want_exit; want_exit="$(jq -r '.expect.exit' "$cf")"
  [[ "$rc" == "$want_exit" ]] || errs+=("exit code $rc, expected $want_exit")

  # findings that must be present: level + substring, and the path if given
  local n i level match fpath line
  n="$(jq -r '.expect.findings | length' "$cf")"
  for ((i=0; i<n; i++)); do
    level="$(jq -r ".expect.findings[$i].level" "$cf")"
    match="$(jq -r ".expect.findings[$i].match" "$cf")"
    fpath="$(jq -r ".expect.findings[$i].path // empty" "$cf")"
    line="$(printf '%s\n' "$out" | grep -F "[$level]" | grep -F "$match" || true)"
    if [[ -z "$line" ]]; then
      errs+=("missing [$level] finding matching: $match")
    elif [[ -n "$fpath" ]] && ! printf '%s\n' "$line" | grep -qF "$fpath"; then
      errs+=("[$level] '$match' did not name the expected path: $fpath")
    fi
  done

  # strings that must never appear. This is the false-positive and the
  # silently-skipped-work guard, and it is the more important half.
  local findings_only
  findings_only="$(printf '%s\n' "$out" | grep -E '\[(HIT|review)\]' || true)"
  while IFS= read -r forbidden; do
    [[ -z "$forbidden" ]] && continue
    printf '%s\n' "$findings_only" | grep -qF "$forbidden" \
      && errs+=("reported as a finding, and must not be: $forbidden")
  done < <(jq -r '.expect.must_not_report[]? // empty' "$cf")

  # the scanned tree must be untouched unless the case says otherwise
  local may_change; may_change="$(jq -r '.expect.tree_may_change // false' "$cf")"
  if [[ "$may_change" != "true" && "$before" != "$after" ]]; then
    errs+=("the scanned tree changed during a read-only run")
  fi

  # --apply moves into quarantine and never deletes
  if [[ "$apply" == "true" ]]; then
    while IFS= read -r q; do
      [[ -z "$q" ]] && continue
      [[ -e "$tmp/tree/$q" ]] && errs+=("still in the tree after --apply: $q")
      find "$qdir" -type f -path "*${q##*/}" 2>/dev/null | grep -q . \
        || errs+=("not found in quarantine after --apply: $q")
    done < <(jq -r '.expect.quarantined[]? // empty' "$cf")
    [[ -f "$qdir/manifest.tsv" ]] || errs+=("--apply wrote no quarantine manifest")
  fi

  if [[ ${#errs[@]} -eq 0 ]]; then
    PASS=$((PASS+1)); printf '  \033[32mpass\033[0m  %-38s %s\n' "$name" "$why"
    [[ $SHOW_DIFF -eq 1 ]] && printf '%s\n' "$out" | sed 's/^/        /'
  else
    FAIL=$((FAIL+1)); FAILED_CASES+=("$name")
    printf '  \033[31mFAIL\033[0m  %-38s %s\n' "$name" "$why"
    printf '        %s\n' "${errs[@]}"
    if [[ -n "$ONLY" || $SHOW_DIFF -eq 1 ]]; then
      printf '        --- run output ---\n'; printf '%s\n' "$out" | sed 's/^/        /'
    fi
  fi
  rm -rf "$tmp"
}

printf 'conformance: %s implementation, indicators from ioc/\n\n' "$IMPL"
for cf in "$HERE"/cases/*.json; do run_case "$cf"; done

printf '\n  %d passed, %d failed\n' "$PASS" "$FAIL"
if [[ $FAIL -gt 0 ]]; then
  printf '  failed: %s\n' "${FAILED_CASES[*]}"
  printf '  one case in detail:  ./conformance/run.sh --case %s\n' "${FAILED_CASES[0]}"
  exit 1
fi
[[ $PASS -eq 0 ]] && { echo "  no cases ran"; exit 3; }
exit 0
