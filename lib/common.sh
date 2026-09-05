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

prc_log() { printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*" >&2; }
prc_die() { printf 'error: %s\n' "$*" >&2; exit 2; }

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
