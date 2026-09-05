#!/usr/bin/env bash
# common.sh - shared functions. Sourced, never executed directly.

# Never let git sit waiting on an interactive credential prompt.
export GIT_TERMINAL_PROMPT=0

PRC_LIB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PRC_ROOT="$(cd "$PRC_LIB/.." && pwd)"
# shellcheck disable=SC2034  # PRC_IOC is read by the scripts that source this file
PRC_IOC="${PRC_IOC_DIR:-$PRC_ROOT/ioc}"

# Obfuscation tells used to qualify a "content after module end" finding.
PRC_TAIL_TELL='eval\(|new Function\(|Buffer\.from\(|child_process|atob\(|fromCharCode|\\x[0-9a-fA-F]{2}\\x[0-9a-fA-F]{2}|require\([^)]*(child_process|https?|net|dns)'

# shellcheck disable=SC2034  # PRC_BENIGN_RE is read by the scripts that source this file
# Paths that contain indicator strings because they are detection tooling, not
# payload. Extend this list if you keep your scanners somewhere else.
PRC_BENIGN_RE='(^|/)\.github/workflows/[^/]*polinrider[^/]*\.(yml|yaml)$'
PRC_BENIGN_RE="$PRC_BENIGN_RE"'|(^|/)\.github/polinrider/'
PRC_BENIGN_RE="$PRC_BENIGN_RE"'|(^|/)(polinrider|scan-workspace|gh-scan|gh-sweep|gh-restore|triage-filter|check-macos|check-linux|check-windows|preflight|selftest|install-workflow|local-common|common)[^/]*\.(sh|ps1)$'
PRC_BENIGN_RE="$PRC_BENIGN_RE"'|(^|/)ioc/[^/]*\.txt$'
PRC_BENIGN_RE="$PRC_BENIGN_RE"'|(^|/)(lib|ci)/'
PRC_BENIGN_RE="$PRC_BENIGN_RE"'|\.md$|(^|/)docs/|README'

# prc_shift_back_2h <iso8601Z> - two hours earlier, same format. Fails loudly.
# T0 for a sweep is conventionally a couple of hours before the first suspect
# push, to catch anything staged just ahead of it.
prc_shift_back_2h() {
  local t="${1:-}" out=""
  # Validate the input, not just the output. GNU date reads " - 2 hours" as a
  # relative offset from now, so an empty or malformed argument comes back as a
  # perfectly well-formed timestamp that is simply wrong, and the output check
  # below cannot tell. BSD date rejects the same input outright, which is how
  # this stayed hidden on macOS.
  case "$t" in
    [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z) ;;
    *) return 1 ;;
  esac
  if date --version >/dev/null 2>&1; then
    out=$(date -u -d "$t - 2 hours" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || true)
  else
    # BSD date: -v must come before -f. The other order silently ignores both
    # the adjustment and the output format and returns the unchanged date in
    # ctime format, which then gets pasted into a --since and fails.
    out=$(date -u -j -v-2H -f "%Y-%m-%dT%H:%M:%SZ" "$t" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || true)
  fi
  case "$out" in
    [0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z) printf '%s\n' "$out" ;;
    *) return 1 ;;
  esac
}

prc_log() { printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; }
prc_die() { printf 'error: %s\n' "$*" >&2; exit 2; }

# --- where evidence goes ----------------------------------------------------
# Mirrors hold attacker-controlled content, so where they live is a security
# decision, not a convenience one. Three requirements, in order:
#
#   1. Outside every git checkout. Inside one, an editor indexes the payload and
#      a stray `git add -A` republishes it from the operator's own account.
#      Cloning a working tree back out of a mirror gives you a live
#      .vscode/tasks.json with "runOn": "folderOpen".
#   2. Gone on its own. A hidden directory under $HOME is a directory nobody
#      looks at again: infected mirrors sit there for months and are still there
#      the next time something walks the filesystem. Temp is cleared on reboot,
#      so forgetting about it is the safe outcome rather than the dangerous one.
#   3. Private. Mode 700, and on macOS $TMPDIR is already per-user.
#
# The cost is real and the tools say so out loud: gh-restore.sh reads the
# pre-attack commit out of the mirror, so once temp is cleared those recovery
# points are gone unless GitHub still serves the unreachable object. Anyone who
# needs the evidence to outlive a reboot passes --out and keeps it deliberately.
prc_default_evidence_dir() {
  local base="${TMPDIR:-/tmp}"
  printf '%s\n' "${POLINRIDER_EVIDENCE_DIR:-${base%/}/polinrider-evidence}"
}

# prc_evidence_is_volatile <dir> - true if this will not survive a reboot.
prc_evidence_is_volatile() {
  case "${1:-}" in
    /tmp/*|/var/tmp/*|/private/tmp/*|/var/folders/*|/private/var/folders/*) return 0 ;;
    *) [[ -n "${TMPDIR:-}" && "${1:-}" == "${TMPDIR%/}"/* ]] ;;
  esac
}

# prc_assert_safe_out <dir> - refuse an evidence directory inside a git checkout.
# Set POLINRIDER_ALLOW_UNSAFE_OUT=1 to override, if you know why you want to.
prc_assert_safe_out() {
  local out="$1" probe top
  [[ "${POLINRIDER_ALLOW_UNSAFE_OUT:-0}" == "1" ]] && return 0

  # Walk up to the nearest directory that exists, then ask git about it.
  probe="$out"
  while [[ ! -d "$probe" && "$probe" != "/" && "$probe" != "." ]]; do
    probe="$(dirname "$probe")"
  done
  top="$(git -C "$probe" rev-parse --show-toplevel 2>/dev/null || true)"
  [[ -z "$top" ]] && return 0

  cat >&2 <<EOF
error: refusing to write evidence into a git working tree.

  requested : $out
  inside    : $top

Mirror clones hold live malware. Inside a checkout your editor indexes them,
and one 'git add -A' republishes the payload from your own account.

Use the default, which is outside every checkout:
  --out $(prc_default_evidence_dir)

Or name somewhere else yourself:
  --out "\$HOME/polinrider-evidence"

Override only if you know why: POLINRIDER_ALLOW_UNSAFE_OUT=1
EOF
  exit 2
}

# prc_evidence_age_days <dir> - whole days since the directory was last written.
prc_evidence_age_days() {
  local d="${1:-}" mtime now
  [[ -d "$d" ]] || return 1
  mtime=$(stat -c %Y "$d" 2>/dev/null || stat -f %m "$d" 2>/dev/null) || return 1
  now=$(date +%s)
  printf '%s\n' $(( (now - mtime) / 86400 ))
}

# prc_evidence_warn_stale <dir> - infected mirrors that outlive the incident are
# a liability: they are still malware, and nobody remembers they are there. Say
# so rather than letting them accumulate quietly.
prc_evidence_warn_stale() {
  local d="${1:-}" age n
  [[ -d "$d" ]] || return 0
  n=$(find "$d" -maxdepth 1 -name '*.git' 2>/dev/null | grep -c . || true)
  [[ "${n:-0}" -gt 0 ]] || return 0
  age=$(prc_evidence_age_days "$d" 2>/dev/null) || return 0
  [[ "${age:-0}" -ge 7 ]] || return 0
  cat >&2 <<EOF

warning: $n infected mirror(s) in $d have not been touched for $age days.
         They are still malware. When the incident is closed, delete them:
           polinrider.sh --purge-evidence
EOF
}

# prc_prepare_out <dir> - validate, create mode 700, echo the absolute path.
prc_prepare_out() {
  local out="$1"
  prc_assert_safe_out "$out"
  mkdir -p "$out" || prc_die "cannot create evidence directory: $out"
  chmod 700 "$out" 2>/dev/null || true
  (cd "$out" && pwd)
}

prc_need() {
  local missing=0 bin
  for bin in "$@"; do
    command -v "$bin" >/dev/null 2>&1 || { printf 'missing dependency: %s\n' "$bin" >&2; missing=1; }
  done
  if [[ $missing -eq 1 ]]; then
    cat >&2 <<'HINT'

Install:
  macOS          brew install gh jq
  Debian/Ubuntu  sudo apt install jq && see https://github.com/cli/cli#installation
  Fedora/RHEL    sudo dnf install gh jq
Then authenticate:
  gh auth login
HINT
    exit 2
  fi
}

# Strip comments and blank lines from an indicator file into $1 (destination).
prc_load_iocs() {
  local dest="$1"; shift
  local src
  : > "$dest"
  for src in "$@"; do
    [[ -f "$src" ]] || prc_die "indicator file not found: $src"
    sed -e 's/[[:space:]]*$//' -e '/^#/d' -e '/^$/d' "$src" >> "$dest"
  done
  [[ -s "$dest" ]] || prc_die "indicator set is empty: $*"
}

# Embed the gh token in the clone URL so a stale system credential helper
# (osxkeychain, credential-manager) can never leave a clone hanging.
prc_clone_url() {
  if [[ -n "${PRC_GH_TOKEN:-}" ]]; then
    printf 'https://x-access-token:%s@github.com/%s.git\n' "$PRC_GH_TOKEN" "$1"
  else
    printf 'https://github.com/%s.git\n' "$1"
  fi
}

prc_cache_token() { PRC_GH_TOKEN="$(gh auth token 2>/dev/null || true)"; }

# prc_list_repos <owner> <org|user> <no-forks 0|1>
prc_list_repos() {
  local owner="$1" kind="$2" nofork="$3" args
  [[ "$kind" == "user" || "$kind" == "org" ]] || prc_die "unknown owner type: $kind"
  args=(--limit 1000 --json nameWithOwner --jq '.[].nameWithOwner')
  [[ "$nofork" -eq 1 ]] && args+=(--source)
  local out
  out="$(gh repo list "$owner" "${args[@]}" 2>/dev/null)" \
    || prc_die "gh repo list $owner failed. Check: gh auth status"
  if [[ "$(printf '%s\n' "$out" | grep -c .)" -ge 1000 ]]; then
    prc_log "WARNING: 1000 repositories returned, which is the listing limit."
    prc_log "         Anything beyond that was NOT scanned. Scan the remainder"
    prc_log "         explicitly with --repo OWNER/NAME."
  fi
  printf '%s\n' "$out"
}

# A repository that fails to clone is never scanned. Silently skipping it means
# the final "INFECTED: 0" covers fewer repositories than the operator thinks.
PRC_CLONE_FAILURES=""

# prc_mirror <owner/name> <dest.git>   mirror-clone and freeze GC
prc_mirror() {
  local nwo="$1" dest="$2"
  if [[ -d "$dest" ]]; then
    prc_log "  mirror exists, skipping clone: $nwo"
  else
    prc_log "  mirror cloning $nwo"
    git clone --mirror "$(prc_clone_url "$nwo")" "$dest" >/dev/null 2>&1 || {
      prc_log "  !! clone FAILED for $nwo (auth or network - check: gh auth status)"
      PRC_CLONE_FAILURES="${PRC_CLONE_FAILURES}${nwo}"$'\n'
      return 1
    }
  fi
  # Orphaned objects are the restore targets. They must survive until you are done.
  git -C "$dest" config gc.auto 0
  git -C "$dest" config gc.pruneExpire never
  git -C "$dest" for-each-ref > "$dest/refs-snapshot.txt" 2>/dev/null
  git -C "$dest" fsck --unreachable --no-progress > "$dest/fsck-unreachable.txt" 2>&1
  return 0
}

# prc_capture_events <owner/name> <events-dir> <ledger.tsv>
# An empty event feed and a failed API call look identical downstream, and during
# an incident that difference decides whether you believe a repository was
# untouched. Record failures so they can be reported instead of assumed empty.
PRC_EVENT_FAILURES=""
prc_capture_events() {
  local nwo="$1" evdir="$2" ledger="$3" name="${1##*/}"
  if ! gh api "/repos/${nwo}/events?per_page=100" --paginate > "$evdir/${name}.json" 2>/dev/null; then
    echo '[]' > "$evdir/${name}.json"
    PRC_EVENT_FAILURES="${PRC_EVENT_FAILURES}${nwo}"$'\n'
    prc_log "  !! events API FAILED for $nwo - treat this repo as UNKNOWN, not clean"
  fi
  jq -r --arg repo "$nwo" '
    (if type=="array" then . else [] end)[]
    | select(.type=="PushEvent")
    | [ $repo,
        (.payload.ref // ""),
        (.payload.before // ""),
        (.payload.head // ""),
        (.actor.login // ""),
        (.created_at // ""),
        ((.payload.size // 0)|tostring),
        (if (.payload.distinct_size // 0) < (.payload.size // 0) then "REVIEW" else "" end)
      ] | @tsv' "$evdir/${name}.json" >> "$ledger" 2>/dev/null
}

# prc_scan_ref <owner/name> <mirror.git> <ref>
# Needs: PRC_STRONG, PRC_WEAK, PRC_FILENAME_RE, PRC_REPORT, PRC_SUMMARY, PRC_FIRST
prc_scan_ref() {
  local repo="$1" dest="$2" ref="$3"
  local hits weak names woff tasks p tail_suspect verdict

  hits="$(git -C "$dest" grep -I -F -n -f "$PRC_STRONG" "$ref" 2>/dev/null | head -50)"
  weak="$(git -C "$dest" grep -I -F -n -f "$PRC_WEAK"   "$ref" 2>/dev/null | head -30)"
  names="$(git -C "$dest" ls-tree -r --name-only "$ref" 2>/dev/null \
           | grep -E -f "$PRC_FILENAME_RE" | head -30)"

  # .vscode/tasks.json is a finding only when it auto-runs on folder open.
  tasks=""
  while read -r p; do
    [[ -z "$p" ]] && continue
    if git -C "$dest" cat-file blob "${ref}:${p}" 2>/dev/null | grep -qF 'folderOpen'; then
      tasks="${tasks}${p} (runOn: folderOpen)"$'\n'
    fi
  done < <(git -C "$dest" ls-tree -r --name-only "$ref" 2>/dev/null \
           | grep -E '(^|/)\.vscode/tasks\.json$' | head -10)
  names="$(printf '%s\n%s' "$names" "$tasks" | grep -v '^[[:space:]]*$' || true)"

  # Font masquerade: .woff/.woff2 whose magic bytes are not wOFF / wOF2.
  woff=""
  while read -r p; do
    [[ -z "$p" ]] && continue
    local head40 magic
    head40="$(git -C "$dest" cat-file blob "${ref}:${p}" 2>/dev/null | head -c 40)"
    # Git LFS stores a text pointer in the blob, not the font. Nothing to inspect.
    [[ "$head40" == version\ https://git-lfs* ]] && continue
    # 0-byte blob (git's empty blob e69de29b...) cannot carry a payload.
    [[ -z "$head40" ]] && continue
    magic="${head40:0:4}"
    case "$magic" in
      wOFF|wOF2) ;;
      *) local hex
         hex="$(git -C "$dest" cat-file blob "${ref}:${p}" 2>/dev/null | head -c 4 \
                | od -An -tx1 | tr -s ' ' | sed 's/^ *//;s/ *$//')"
         woff="${woff}${p} (first bytes: ${hex})"$'\n' ;;
    esac
  done < <(git -C "$dest" ls-tree -r --name-only "$ref" 2>/dev/null \
           | grep -Ei '\.woff2?$' \
           | grep -Ev '(^|/)__MACOSX/|(^|/)\._' | head -40)

  # Build config with content after the apparent module end.
  tail_suspect=""
  while read -r p; do
    [[ -z "$p" ]] && continue
    local body last_export total
    body="$(git -C "$dest" cat-file blob "${ref}:${p}" 2>/dev/null)"
    total="$(printf '%s\n' "$body" | wc -l | tr -d ' ')"
    last_export="$(printf '%s\n' "$body" | grep -n -E '^(export default|module\.exports)' | tail -1 | cut -d: -f1)"
    if [[ -n "$last_export" && "$total" -gt $(( last_export + 15 )) ]]; then
      # Content after the module end is normal in flat configs. It is only a
      # signal when that content also looks like a payload.
      local rest
      rest="$(printf '%s\n' "$body" | tail -n +$(( last_export + 1 )))"
      if printf '%s' "$rest" | grep -qE "$PRC_TAIL_TELL" \
         || printf '%s' "$rest" | awk 'length($0) > 500 {found=1} END{exit !found}'; then
        tail_suspect="${tail_suspect}${p} (${total} lines, module end at ${last_export}, payload-shaped remainder)"$'\n'
      fi
    fi
  done < <(git -C "$dest" ls-tree -r --name-only "$ref" 2>/dev/null \
           | grep -E '((postcss|tailwind|eslint|vite|next|rollup|webpack|babel|gridsome|vue)\.config\.(js|mjs|cjs|ts)|truffle\.js)$' | head -20)

  woff="${woff%$'\n'}"
  tail_suspect="${tail_suspect%$'\n'}"

  verdict="clean"
  [[ -n "$hits" || -n "$names" || -n "$woff" ]] && verdict="INFECTED"
  [[ "$verdict" == "clean" && ( -n "$weak" || -n "$tail_suspect" ) ]] && verdict="review"

  # Matched content comes from files an attacker controls. Strip control
  # characters before they reach a terminal or the report.
  hits="$(printf '%s' "$hits" | LC_ALL=C tr -d '\000-\010\013\014\016-\037\177')"
  weak="$(printf '%s' "$weak" | LC_ALL=C tr -d '\000-\010\013\014\016-\037\177')"

  if [[ "$verdict" != "clean" ]]; then
    { echo "### ${repo}  ${ref}  -> ${verdict}"
      [[ -n "$hits" ]]         && { echo "  [IOC strings]";     echo "$hits"         | sed 's/^/    /'; }
      [[ -n "$names" ]]        && { echo "  [IOC filenames]";   echo "$names"        | sed 's/^/    /'; }
      [[ -n "$woff" ]]         && { echo "  [font masquerade]"; echo "$woff"         | sed 's/^/    /'; }
      [[ -n "$weak" ]]         && { echo "  [weak signals]";    echo "$weak"         | sed 's/^/    /'; }
      [[ -n "$tail_suspect" ]] && { echo "  [config tail]";     echo "$tail_suspect" | sed 's/^/    /'; }
      echo
    } >> "$PRC_SUMMARY"
  fi

  [[ "$PRC_FIRST" -eq 0 ]] && echo ',' >> "$PRC_REPORT"
  PRC_FIRST=0
  jq -n --arg repo "$repo" --arg ref "$ref" --arg verdict "$verdict" \
        --arg hits "$hits" --arg names "$names" --arg woff "$woff" \
        --arg weak "$weak" --arg tail "$tail_suspect" \
    '{repo:$repo, ref:$ref, verdict:$verdict,
      ioc_strings:($hits|split("\n")|map(select(length>0))),
      ioc_filenames:($names|split("\n")|map(select(length>0))),
      font_masquerade:($woff|split("\n")|map(select(length>0))),
      weak_signals:($weak|split("\n")|map(select(length>0))),
      config_tail:($tail|split("\n")|map(select(length>0)))}' >> "$PRC_REPORT"
}
