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
#
# HOST CASES. A case with a "host" key describes a machine as well as a tree:
# its process table, sockets, crontab and system directories, as files. They
# run without --fs-only and with --host-state pointing at those files. Only the
# Rust engine can be handed a machine that does not exist, so under the shell
# implementation these print "skip" and are counted, never silently dropped.
#
# GUIDE CASES. A case with "command": "guide" runs the guided flow and feeds it
# the case's "stdin", one answer per line. {{TREE}} in an answer is the
# fixture tree, so a case can type a path it could not know in advance.
#
# GITHUB CASES. A case with a "forge" key describes repositories on a pretend
# GitHub: owners, their repositories, branches and files. The runner builds
# them as real git repositories and hands the directory to the guided flow
# with --forge-state. "forge_files" adds plain files beside them: who is
# signed in, the list of organizations, a marker that gh is not installed.
# A repository's "attacks" are pushes made after it existed, each recorded as
# GitHub would record it. Afterwards every repository must be exactly as it
# was, unless "expect.forge_after" names it and says what it should hold: a
# case that pushes has to say so, and say what the push should have done.
#
# CLEAN CASES. A case with "command": "clean" runs the clean command, which
# strips an appended payload out of a build config in place. The shell has no
# such command, so these skip under it too, the same way.

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
# For the host cases: an implant process name, the same name as the Linux
# kernel reports it (15 bytes, no more), and a campaign address.
IMPLANT="$(pick implant-names.txt 1)"
IMPLANT_CUT="${IMPLANT:0:15}"
NETIP="$(sed -e '/^#/d' -e '/^$/d' "$ROOT/ioc/network.txt" | grep -E '^[0-9]+(\.[0-9]+){3}$' | sed -n 1p)"
[[ -n "$IMPLANT" && -n "$NETIP" ]] || { echo "implant-names.txt or network.txt is empty or unreadable" >&2; exit 3; }
# The cut-name case is only a test if the name is longer than the cut. If the
# first entry ever stops being, fail here rather than pass a case that proves
# nothing.
[[ ${#IMPLANT} -gt 15 ]] || { echo "the first implant name is not longer than 15 bytes; the kernel-truncation case needs one that is" >&2; exit 3; }
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
  # $state belongs to the shell engine. Discarded here, at the top: putting
  # it after the run made ":" the last command, so rc captured its exit status
  # instead of the binary's and every case read as exit 0.
  : "$state"
  # No --state flag either: 2.0 has no checkpointing (ADR-0030) and the Rust
  # engine refuses the flag rather than ignoring it. --home is explicit so the scan reads
  # the fixture's home rather than the runner's.
  local args
  if [[ "$COMMAND" == "guide" ]]; then
    # The guided flow takes no roots and no --apply: it asks. The answers come
    # from the case. The quarantine is always named, because whether anything
    # goes into it is decided by an answer and not by a flag.
    args=(guide --home "$FAKE_HOME" --report "$report" --quarantine "$(dirname "$report")/quarantine")
    [[ -n "$HOST_STATE" ]] && args+=(--host-state "$HOST_STATE")
    # Copies of repositories go beside the report, which is outside every
    # checkout: the engine refuses anywhere that is not.
    [[ -n "$FORGE_STATE" ]] && args+=(--forge-state "$FORGE_STATE" --evidence "$(dirname "$report")/evidence")
    jq -r '.stdin // [] | .[]' "$CASE_FILE" | sed -e "s|{{TREE}}|$TREE|g" | "$bin" "${args[@]}" 2>&1
    return "${PIPESTATUS[2]}"
  fi
  if [[ "$COMMAND" == "clean" ]]; then
    # clean reads the roots and nothing else: no home, no host, and it refuses
    # the flags that would say otherwise.
    args=(clean --report "$report")
  else
    args=(check --home "$FAKE_HOME" --report "$report")
    # A host case supplies the machine; every other case reads no host state.
    if [[ -n "$HOST_STATE" ]]; then args+=(--host-state "$HOST_STATE"); else args+=(--fs-only); fi
  fi
  [[ -n "$qdir" ]] && args+=(--apply --quarantine "$qdir")
  "$bin" "${args[@]}" "$@" 2>&1
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
        -e "s|{{BADPKG}}|$BADPKG|g" -e "s|{{PAD}}|$PAD|g" \
        -e "s|{{IMPLANT_CUT}}|$IMPLANT_CUT|g" -e "s|{{IMPLANT}}|$IMPLANT|g" \
        -e "s|{{NETIP}}|$NETIP|g"; }

# build_files <case file> <key> <destination>: write every entry of one of the
# case's file maps under a directory.
build_files() {
  local cf="$1" key="$2" dest="$3" path content
  while IFS= read -r path; do
    content="$(jq -r --arg k "$key" --arg p "$path" '.[$k][$p]' "$cf")"
    mkdir -p "$dest/$(dirname "$path")"
    subst "$content" > "$dest/$path"
  done < <(jq -r --arg k "$key" '.[$k] // {} | keys[]' "$cf")
}

# build_forge <case file> <destination>: real git repositories for every
# owner/repository/branch the case describes. Identity and signing are set on
# the command line, so this works on a machine with no git configuration.
build_forge() {
  local cf="$1" dest="$2" owner repo branch path content work
  local bare n i a claimed amend size before head actor
  local g=(git -c user.name=fixture -c user.email=fixture@localhost -c commit.gpgsign=false)
  mkdir -p "$dest/repos" "$dest/git" "$dest/pushes"
  build_files "$cf" forge_files "$dest"
  while IFS= read -r owner; do
    : > "$dest/repos/$owner"
    while IFS= read -r repo; do
      printf '%s/%s\n' "$owner" "$repo" >> "$dest/repos/$owner"
      # Listed and not there: a repository that cannot be copied.
      [[ "$(jq -r --arg o "$owner" --arg r "$repo" '.forge[$o][$r].missing // false' "$cf")" == "true" ]] && continue
      work="$dest/.work/$owner/$repo"; mkdir -p "$work"
      "${g[@]}" -C "$work" init -q -b main
      printf 'base\n' > "$work/BASE.txt"
      "${g[@]}" -C "$work" add -A; "${g[@]}" -C "$work" commit -q -m base
      while IFS= read -r branch; do
        if [[ "$branch" == "main" ]]; then "${g[@]}" -C "$work" checkout -q main
        else "${g[@]}" -C "$work" checkout -q -b "$branch" main; fi
        while IFS= read -r path; do
          content="$(jq -r --arg o "$owner" --arg r "$repo" --arg b "$branch" --arg p "$path" \
                     '.forge[$o][$r].branches[$b][$p]' "$cf")"
          mkdir -p "$work/$(dirname "$path")"
          subst "$content" > "$work/$path"
        done < <(jq -r --arg o "$owner" --arg r "$repo" --arg b "$branch" '.forge[$o][$r].branches[$b] | keys[]' "$cf")
        "${g[@]}" -C "$work" add -A; "${g[@]}" -C "$work" commit -q -m change
      done < <(jq -r --arg o "$owner" --arg r "$repo" '.forge[$o][$r].branches // {} | keys[]' "$cf")
      mkdir -p "$dest/git/$owner" "$dest/pushes/$owner"
      "${g[@]}" -C "$work" checkout -q main
      "${g[@]}" clone -q --bare "$work" "$dest/git/$owner/$repo.git"
      bare="$dest/git/$owner/$repo.git"
      # GitHub serves a commit by its ID whether or not a branch still
      # reaches it. A plain git repository has to be told to.
      "${g[@]}" -C "$bare" config uploadpack.allowAnySHA1InWant true
      content="$(jq -r --arg o "$owner" --arg r "$repo" '.forge[$o][$r].pushes // empty' "$cf")"
      [[ -n "$content" ]] && printf '%s\n' "$content" > "$dest/pushes/$owner/$repo.tsv"
      # Pushes made after the repository existed, each a real push to the
      # pretend GitHub. "force" replaces the newest commit, which leaves the
      # one it replaced reachable from nothing, as a force-push does. With
      # an "actor" the push goes on the record, holding the commit the
      # branch pointed to before it. "date" is what the commit CLAIMS as
      # its date, which is the attacker's to choose.
      n="$(jq -r --arg o "$owner" --arg r "$repo" '.forge[$o][$r].attacks // [] | length' "$cf")"
      for ((i=0; i<n; i++)); do
        a=".forge[\$o][\$r].attacks[$i]"
        branch="$(jq -r --arg o "$owner" --arg r "$repo" "$a.branch" "$cf")"
        "${g[@]}" -C "$work" checkout -q "$branch"
        while IFS= read -r path; do
          content="$(jq -r --arg o "$owner" --arg r "$repo" --arg p "$path" "$a.files[\$p]" "$cf")"
          mkdir -p "$work/$(dirname "$path")"
          subst "$content" > "$work/$path"
        done < <(jq -r --arg o "$owner" --arg r "$repo" "$a.files | keys[]" "$cf")
        "${g[@]}" -C "$work" add -A
        claimed="$(jq -r --arg o "$owner" --arg r "$repo" "$a.date // empty" "$cf")"
        amend=(); size=1
        if [[ "$(jq -r --arg o "$owner" --arg r "$repo" "$a.force // false" "$cf")" == "true" ]]; then
          amend=(--amend); size=0
        fi
        if [[ -n "$claimed" ]]; then
          GIT_AUTHOR_DATE="$claimed" GIT_COMMITTER_DATE="$claimed" \
            "${g[@]}" -C "$work" commit -q ${amend[@]+"${amend[@]}"} -m "update config"
        else
          "${g[@]}" -C "$work" commit -q ${amend[@]+"${amend[@]}"} -m "update config"
        fi
        before="$("${g[@]}" -C "$bare" rev-parse "refs/heads/$branch")"
        "${g[@]}" -C "$work" push -q --force "$bare" "$branch:$branch"
        head="$("${g[@]}" -C "$bare" rev-parse "refs/heads/$branch")"
        actor="$(jq -r --arg o "$owner" --arg r "$repo" "$a.actor // empty" "$cf")"
        [[ -n "$actor" ]] && printf 'refs/heads/%s\t%s\t%s\t%s\t%s\t%s\n' "$branch" "$before" "$head" "$actor" \
          "$(jq -r --arg o "$owner" --arg r "$repo" "$a.at" "$cf")" "$size" >> "$dest/pushes/$owner/$repo.tsv"
      done
    done < <(jq -r --arg o "$owner" '.forge[$o] | keys[]' "$cf")
  done < <(jq -r '.forge // {} | keys[]' "$cf")
  rm -rf "$dest/.work"
}

# forge_refs <forge> <owner/repo>: where every branch and tag of one
# repository points, and what was set on it. Empty for one that is not there.
forge_refs() {
  git -C "$1/git/$2.git" for-each-ref --format='%(refname) %(objectname)' 2>/dev/null || true
  ls "$1/changed/$2".* 2>/dev/null || true
}

# forge_repos <case file>: owner/repo for every repository of the case.
forge_repos() { jq -r '.forge // {} | to_entries[] | .key as $o | .value | keys[] | "\($o)/\(.)"' "$1"; }

PASS=0; FAIL=0; SKIP=0; FAILED_CASES=()
FORGE_STATE=""
HOST_STATE=""
COMMAND="check"
CASE_FILE=""
TREE=""

run_case() {
  local cf="$1" name; name="$(basename "$cf" .json)"
  [[ -n "$ONLY" && "$ONLY" != "$name" ]] && return 0

  local why; why="$(jq -r '.why' "$cf")"

  # A host case needs an engine that can be handed a machine. Said, and
  # counted, so a shell run never looks as though it covered them.
  local is_host; is_host="$(jq -r 'has("host")' "$cf")"
  if [[ "$is_host" == "true" && "$IMPL" != "rust" ]]; then
    SKIP=$((SKIP+1))
    printf '  \033[33mskip\033[0m  %-38s %s\n' "$name" "needs supplied host state, which only the Rust engine takes"
    return 0
  fi

  COMMAND="$(jq -r '.command // "check"' "$cf")"
  if [[ "$COMMAND" != "check" && "$IMPL" != "rust" ]]; then
    SKIP=$((SKIP+1))
    printf '  \033[33mskip\033[0m  %-38s %s\n' "$name" "runs the $COMMAND command, which only the Rust engine has"
    return 0
  fi

  local tmp; tmp="$(mktemp -d)"
  FAKE_HOME="$tmp/home"; mkdir -p "$FAKE_HOME" "$tmp/tree" "$tmp/out"

  CASE_FILE="$cf"; TREE="$tmp/tree"

  # build the fixture: the tree to scan, the home directory, and for a host
  # case the machine's state
  build_files "$cf" files "$tmp/tree"
  build_files "$cf" home "$FAKE_HOME"
  FORGE_STATE=""
  if [[ "$(jq -r 'has("forge") or has("forge_files")' "$cf")" == "true" ]]; then
    FORGE_STATE="$tmp/forge"
    build_forge "$cf" "$FORGE_STATE"
  fi
  HOST_STATE=""
  if [[ "$is_host" == "true" ]]; then
    HOST_STATE="$tmp/host"; mkdir -p "$HOST_STATE"
    build_files "$cf" host "$HOST_STATE"
  fi

  # roots, relative to the fixture tree
  local roots=(); local r
  while IFS= read -r r; do roots+=("$tmp/tree/$r"); done < <(jq -r '.roots[]' "$cf")

  local apply qdir=""
  apply="$(jq -r '.apply // false' "$cf")"
  [[ "$apply" == "true" ]] && qdir="$tmp/out/quarantine"

  # For a host case the home directory and the supplied state are part of what
  # a read-only run must leave alone: that is where persistence lives.
  local before after out rc nwo
  if [[ -n "$FORGE_STATE" ]]; then
    mkdir -p "$tmp/forge-before"
    while IFS= read -r nwo; do
      forge_refs "$FORGE_STATE" "$nwo" > "$tmp/forge-before/${nwo//\//__}"
    done < <(forge_repos "$cf")
  fi
  before="$(snapshot "$tmp/tree")"
  [[ -n "$HOST_STATE" ]] && before+="$(snapshot "$FAKE_HOME")$(snapshot "$HOST_STATE")"
  out="$("impl_$IMPL" "$tmp/out/report.txt" "$tmp/out/state" "$qdir" "${roots[@]}")"
  rc=$?
  after="$(snapshot "$tmp/tree")"
  [[ -n "$HOST_STATE" ]] && after+="$(snapshot "$FAKE_HOME")$(snapshot "$HOST_STATE")"

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

  # strings that must appear nowhere at all, on the console or in the report
  # file. This is the redaction guard: a secret the scan read must not be a
  # secret the scan wrote down.
  local everything="$out"
  [[ -f "$tmp/out/report.txt" ]] && everything+="$(cat "$tmp/out/report.txt")"
  while IFS= read -r forbidden; do
    [[ -z "$forbidden" ]] && continue
    printf '%s\n' "$everything" | grep -qF "$forbidden" \
      && errs+=("printed, and must never be: $forbidden")
  done < <(jq -r '.expect.must_not_print[]? // empty' "$cf")

  # strings that must appear somewhere in the output. For advice printed under
  # a finding, which carries no level tag of its own to match on.
  while IFS= read -r wanted; do
    [[ -z "$wanted" ]] && continue
    printf '%s\n' "$out" | grep -qF "$wanted" \
      || errs+=("not printed, and must be: $wanted")
  done < <(jq -r '.expect.must_print[]? // empty' "$cf")

  # files that must hold exactly this afterwards. Compared without trailing
  # newlines, which the fixture builder does not control on every platform.
  local want_path want_body
  while IFS= read -r want_path; do
    [[ -z "$want_path" ]] && continue
    want_body="$(subst "$(jq -r --arg p "$want_path" '.expect.file_after[$p]' "$cf")")"
    if [[ ! -f "$tmp/tree/$want_path" ]]; then
      errs+=("missing afterwards: $want_path")
    elif [[ "$(cat "$tmp/tree/$want_path")" != "$want_body" ]]; then
      errs+=("not what it should be afterwards: $want_path")
    fi
  done < <(jq -r '.expect.file_after // {} | keys[]' "$cf")

  # the scanned tree must be untouched unless the case says otherwise
  local may_change; may_change="$(jq -r '.expect.tree_may_change // false' "$cf")"
  if [[ "$may_change" != "true" && "$before" != "$after" ]]; then
    errs+=("the scanned tree changed during a read-only run")
  fi

  # The pretend GitHub. A repository the case does not name under
  # "forge_after" must be exactly as it was: every branch, every tag, its
  # description and whether it is archived. This is the guard on the half of
  # the tool that pushes, and it is on by default.
  if [[ -n "$FORGE_STATE" ]]; then
    local fa bare want got
    while IFS= read -r nwo; do
      fa="$(jq -c --arg n "$nwo" '.expect.forge_after[$n] // empty' "$cf")"
      bare="$FORGE_STATE/git/$nwo.git"
      if [[ -z "$fa" || "$(jq -r '.unchanged // false' <<<"$fa")" == "true" ]]; then
        [[ "$(forge_refs "$FORGE_STATE" "$nwo")" == "$(cat "$tmp/forge-before/${nwo//\//__}")" ]] \
          || errs+=("$nwo was changed on the pretend GitHub, and must not be")
        continue
      fi
      while IFS= read -r want; do
        git -C "$bare" grep -qF "$STRONG" "refs/heads/$want" 2>/dev/null \
          && errs+=("$nwo: $want still carries the indicator")
      done < <(jq -r '.clean[]? // empty' <<<"$fa")
      while IFS= read -r want; do
        git -C "$bare" grep -qF "$STRONG" "refs/heads/$want" 2>/dev/null \
          || errs+=("$nwo: $want no longer carries the indicator, and the case says it stays")
      done < <(jq -r '.infected[]? // empty' <<<"$fa")
      # Read whole and then searched: with pipefail on, a grep -q that stops
      # at its first match fails the pipe, and found would read as not found.
      got="$(git -C "$bare" log --branches --tags -p --format=%H 2>/dev/null)"
      case "$(jq -r '.history // empty' <<<"$fa")" in
        clean)    grep -qF "$STRONG" <<<"$got" \
                    && errs+=("$nwo: the indicator is still in the history") ;;
        infected) grep -qF "$STRONG" <<<"$got" \
                    || errs+=("$nwo: the history was rewritten, and the case says it is not") ;;
      esac
      while IFS= read -r want; do
        got="$(git -C "$bare" cat-file blob "$want" 2>/dev/null)" \
          || { errs+=("$nwo: missing afterwards: $want"); continue; }
        [[ "$got" == "$(subst "$(jq -r --arg k "$want" '.file[$k]' <<<"$fa")")" ]] \
          || errs+=("$nwo: not what it should be afterwards: $want")
      done < <(jq -r '.file // {} | keys[]' <<<"$fa")
      want="$(jq -r '.readme_top // empty' <<<"$fa")"
      if [[ -n "$want" ]]; then
        got="$(git -C "$bare" cat-file blob HEAD:README.md 2>/dev/null | sed -n 1p)"
        [[ "$got" == "$want" ]] || errs+=("$nwo: the README does not open with the notice")
      fi
      want="$(jq -r '.readme_keeps // empty' <<<"$fa")"
      if [[ -n "$want" ]]; then
        got="$(git -C "$bare" cat-file blob HEAD:README.md 2>/dev/null)"
        grep -qF "$want" <<<"$got" \
          || errs+=("$nwo: the README lost what it held before: $want")
      fi
      if [[ "$(jq -r '.archived // false' <<<"$fa")" == "true" ]]; then
        [[ -f "$FORGE_STATE/changed/$nwo.archived" ]] || errs+=("$nwo: was not archived")
        grep -qF "INFECTED" "$FORGE_STATE/changed/$nwo.description" 2>/dev/null \
          || errs+=("$nwo: the description was not replaced")
      else
        [[ -e "$FORGE_STATE/changed/$nwo.archived" ]] && errs+=("$nwo: was archived, and must not be")
      fi
    done < <(forge_repos "$cf")
  fi

  # --apply moves into quarantine and never deletes
  if [[ "$apply" == "true" ]]; then
    while IFS= read -r q; do
      [[ -z "$q" ]] && continue
      [[ -e "$tmp/tree/$q" ]] && errs+=("still in the tree after --apply: $q")
      find "$qdir" -type f -path "*${q##*/}" 2>/dev/null | grep -q . \
        || errs+=("not found in quarantine after --apply: $q")
    done < <(jq -r '.expect.quarantined[]? // empty' "$cf")
    # a stripped file stays where it was, without the indicator, and the
    # original, with it, is in quarantine
    while IFS= read -r q; do
      [[ -z "$q" ]] && continue
      [[ -f "$tmp/tree/$q" ]] || { errs+=("stripped file is gone from the tree: $q"); continue; }
      grep -qF "$STRONG" "$tmp/tree/$q" && errs+=("still carries the indicator after the strip: $q")
      local kept; kept="$(find "$qdir" -type f -path "*${q##*/}" 2>/dev/null | sed -n 1p)"
      if [[ -z "$kept" ]]; then errs+=("original not kept in quarantine: $q")
      elif ! grep -qF "$STRONG" "$kept"; then errs+=("the copy in quarantine is not the infected original: $q"); fi
    done < <(jq -r '.expect.stripped[]? // empty' "$cf")
    # the same, for an artifact that lived in the home directory
    while IFS= read -r q; do
      [[ -z "$q" ]] && continue
      [[ -e "$FAKE_HOME/$q" ]] && errs+=("still in the home directory after --apply: $q")
      find "$qdir" -type f -path "*${q##*/}" 2>/dev/null | grep -q . \
        || errs+=("not found in quarantine after --apply: $q")
    done < <(jq -r '.expect.home_quarantined[]? // empty' "$cf")
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

# --- refusals -------------------------------------------------------------
# Only the Rust engine promises these. The shell accepts some of them by
# ignoring them, which is the behaviour being replaced.
refusals() {
  [[ "$IMPL" != "rust" ]] && return 0
  local bin="${CARGO_TARGET_DIR:-$ROOT/target}/release/polinrider"
  [[ -x "$bin" ]] || return 0
  local tmp; tmp="$(mktemp -d)"

  check_refusal() {
    local why="$1"; shift
    local out rc
    out="$("$bin" "$@" 2>&1)"; rc=$?
    if [[ $rc -eq 3 ]]; then
      PASS=$((PASS+1)); printf '  \033[32mpass\033[0m  %-38s %s\n' "refuses" "$why"
    else
      FAIL=$((FAIL+1)); FAILED_CASES+=("refuse:$why")
      printf '  \033[31mFAIL\033[0m  %-38s %s\n' "refuses" "$why"
      printf '        exit %s, expected 3\n        %s\n' "$rc" "${out%%$'\n'*}"
    fi
  }

  check_refusal "an unknown flag"            check --not-a-real-flag "$tmp"
  check_refusal "a flag it has not built"    check --jobs 4 "$tmp"
  check_refusal "a flag 2.0 removed, --state"        check --state /tmp/s "$tmp"
  check_refusal "a flag 2.0 removed, --resume"       check --resume "$tmp"
  check_refusal "a root that does not exist" check "$tmp/definitely-absent"
  check_refusal "no root at all"             check --fs-only
  check_refusal "a missing indicator set"    check --ioc "$tmp/no-ioc-here" "$tmp"
  check_refusal "a root that is a file"      check "$0"
  check_refusal "host state that is not there"       check --host-state "$tmp/no-state-here" "$tmp"
  check_refusal "host state alongside --fs-only"     check --fs-only --host-state "$tmp" "$tmp"
  check_refusal "host state that names no platform"  check --host-state "$tmp" "$tmp"
  check_refusal "the guided flow told to --apply"    guide --apply
  check_refusal "the guided flow handed a directory" guide "$tmp"
  check_refusal "clean with no directory"            clean

  # A reader that goes away. `polinrider | head` is an ordinary thing to type,
  # and Rust's println! panics on a closed pipe. The run must end without a
  # word on stderr and with the exit code it would have had anyway. The
  # reader here closes its end at once and stays alive, so the write that
  # fails is the binary's and not a race the binary usually wins.
  check_closed_pipe() {
    local why="$1" want="$2"; shift 2
    local err rc
    err="$( { "$bin" "$@" | { exec 0<&-; sleep 1; }; exit "${PIPESTATUS[0]}"; } 2>&1 >/dev/null )"; rc=$?
    if [[ -z "$err" && $rc -eq $want ]]; then
      PASS=$((PASS+1)); printf '  \033[32mpass\033[0m  %-38s %s\n' "closed pipe" "$why"
    else
      FAIL=$((FAIL+1)); FAILED_CASES+=("pipe:$why")
      printf '  \033[31mFAIL\033[0m  %-38s %s\n' "closed pipe" "$why"
      printf '        exit %s, expected %s\n        %s\n' "$rc" "$want" "${err%%$'\n'*}"
    fi
  }
  mkdir -p "$tmp/empty"
  check_closed_pipe "--version, nobody reading"       0 --version
  check_closed_pipe "a clean scan, nobody reading"    0 check --fs-only "$tmp/empty"
  check_refusal "clean handed host state"            clean --host-state "$tmp" "$tmp"
  check_refusal "clean handed --fs-only"             clean --fs-only "$tmp"
  rm -rf "$tmp"
}

printf 'conformance: %s implementation, indicators from ioc/\n\n' "$IMPL"
for cf in "$HERE"/cases/*.json; do run_case "$cf"; done
[[ -z "$ONLY" ]] && refusals

if [[ $SKIP -gt 0 ]]; then
  printf '\n  %d passed, %d failed, %d skipped\n' "$PASS" "$FAIL" "$SKIP"
else
  printf '\n  %d passed, %d failed\n' "$PASS" "$FAIL"
fi
if [[ $FAIL -gt 0 ]]; then
  printf '  failed: %s\n' "${FAILED_CASES[*]}"
  printf '  one case in detail:  ./conformance/run.sh --case %s\n' "${FAILED_CASES[0]}"
  exit 1
fi
[[ $PASS -eq 0 ]] && { echo "  no cases ran"; exit 3; }
exit 0
