#!/usr/bin/env bash
# selftest-rewrite.sh - offline tests for --rewrite, against both backends.
#
# The additive mode leaves the payload reachable: anyone can check out an older
# commit and get a live .vscode/tasks.json that runs on folder open. --rewrite
# takes it out of every commit instead. This proves it does that, and that it
# does not take anything else with it.
#
# git filter-repo is used when installed and git filter-branch otherwise. Both
# paths are exercised here; the filter-repo half skips if it is not available.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PASS=0; FAIL=0
ok(){ PASS=$((PASS+1)); printf '  ok   %s\n' "$*"; }
no(){ FAIL=$((FAIL+1)); printf '  FAIL %s\n' "$*"; }
export GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@e GIT_COMMITTER_NAME=t GIT_COMMITTER_EMAIL=t@e

# build_fixture <dir> - an origin whose whole history carries the payload,
# on two branches and one annotated tag, plus real content that must survive.
build_fixture() {
  local T="$1" i
  local O="$T/origin" W="$T/w"
  git init -q --bare "$O"
  git init -q "$W"; git -C "$W" checkout -q -b master
  mkdir -p "$W/public/fonts" "$W/.vscode"
  for i in 1 2 3; do
    echo "line $i" >> "$W/app.js"
    printf '        var _0=1;// rmcej%%otb%%\n' > "$W/public/fonts/fa-solid-400.woff2"
    printf '{"tasks":[{"runOptions":{"runOn":"folderOpen"}}]}' > "$W/.vscode/tasks.json"
    git -C "$W" add -A; git -C "$W" commit -qm "commit $i"
  done
  git -C "$W" checkout -q -b dev; echo x >> "$W/app.js"; git -C "$W" commit -qam dev
  git -C "$W" tag -a v1 -m v1
  git -C "$W" remote add origin "$O"; git -C "$W" push -q origin master dev --tags
  mkdir -p "$T/ev/clean"
  printf 'x/origin\t.vscode/tasks.json\nx/origin\tpublic/fonts/fa-solid-400.woff2\n' > "$T/ev/affected-paths.tsv"
  printf 'x/origin\trefs/heads/master\nx/origin\trefs/heads/dev\n' > "$T/ev/affected-refs.tsv"
  git clone -q --bare "file://$O" "$T/ev/clean/x_origin.git"
  git -C "$T/ev/clean/x_origin.git" remote set-url origin "$O"
}

payload_count() { git -C "$1" rev-list --branches --tags --count -- \
  .vscode/tasks.json public/fonts/fa-solid-400.woff2 2>/dev/null || echo 99; }

run_backend() {  # run_backend <name>
  local B="$1" T O M out
  T="$(mktemp -d "${TMPDIR:-/tmp}/prc-rw.XXXXXX")"; O="$T/origin"; M="$T/ev/clean/x_origin.git"
  build_fixture "$T"
  echo "== $B =="
  [[ "$(payload_count "$O")" -gt 0 ]] && ok "$B: fixture starts infected" || no "$B: fixture starts infected"

  # Dry run must change nothing at all.
  local before after
  before="$(git -C "$O" rev-parse master)"
  out="$(PRC_REWRITE_BACKEND="$B" POLINRIDER_ALLOW_UNSAFE_OUT=1 \
    "$ROOT/lib/gh-clean.sh" x/origin --rewrite --out "$T/ev" 2>&1)"
  after="$(git -C "$O" rev-parse master)"
  [[ "$before" == "$after" ]] && ok "$B: dry run pushes nothing" || no "$B: dry run pushes nothing"
  grep -q "backend        : $B" <<<"$out" && ok "$B: reports the backend it will use" || no "$B: reports the backend it will use"
  grep -q 'does not remove anything from GitHub' <<<"$out" \
    && ok "$B: dry run states the GitHub retention caveat" || no "$B: dry run states the GitHub retention caveat"
  grep -q 'Every commit id in the repository changes' <<<"$out" \
    && ok "$B: dry run states that every SHA changes" || no "$B: dry run states that every SHA changes"

  out="$(PRC_REWRITE_BACKEND="$B" POLINRIDER_ALLOW_UNSAFE_OUT=1 \
    "$ROOT/lib/gh-clean.sh" x/origin --rewrite --apply --out "$T/ev" 2>&1)"

  [[ "$(payload_count "$O")" -eq 0 ]] && ok "$B: payload gone from every branch and tag" \
                                      || no "$B: payload gone from every branch and tag"
  git -C "$O" ls-tree -r --name-only master | grep -q '^app.js$' \
    && ok "$B: real content survives" || no "$B: real content survives"
  [[ "$(git -C "$O" rev-list --count master)" -eq 3 ]] \
    && ok "$B: commit count preserved, nothing squashed away" || no "$B: commit count preserved"
  [[ -n "$(git -C "$O" for-each-ref refs/heads/dev)" ]] \
    && ok "$B: other branches still exist" || no "$B: other branches still exist"
  [[ -n "$(git -C "$O" for-each-ref refs/tags/v1)" ]] \
    && ok "$B: the annotated tag survives" || no "$B: the annotated tag survives"
  git -C "$O" ls-tree -r --name-only 'v1^{commit}' 2>/dev/null | grep -q 'tasks.json' \
    && no "$B: the tag no longer points at infected content" \
    || ok "$B: the tag no longer points at infected content"
  ! grep -q 'NOT UPDATED' <<<"$out" && ok "$B: every ref verified on the remote" || no "$B: every ref verified on the remote"
  [[ -n "$(git -C "$M" for-each-ref 'refs/polinrider/pre-rewrite/**')" ]] \
    && ok "$B: pre-rewrite refs kept locally" || no "$B: pre-rewrite refs kept locally"
  grep -q 'contact GitHub Support' <<<"$out" \
    && ok "$B: says the old commits are still fetchable" || no "$B: says the old commits are still fetchable"
  rm -rf "$T"
}

if git filter-repo --version >/dev/null 2>&1; then
  run_backend filter-repo
else
  echo "== filter-repo =="; echo "  skip  git filter-repo is not installed"
fi
run_backend filter-branch

echo
echo "== the two modes stay distinct =="
grep -q 'push .*--force' "$ROOT/lib/gh-clean.sh" \
  && ok "rewrite mode force-pushes, as it must" || no "rewrite mode force-pushes, as it must"
grep -qE 'git -C "\$WORK" push --quiet origin "\$ref:\$ref"' "$ROOT/lib/gh-clean.sh" \
  && ok "additive mode still uses a plain push" || no "additive mode still uses a plain push"

printf '\n  passed %s, failed %s\n' "$PASS" "$FAIL"
[[ "$FAIL" -eq 0 ]] || exit 1
