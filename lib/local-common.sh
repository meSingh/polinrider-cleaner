#!/usr/bin/env bash
# local-common.sh - checks shared by check-macos.sh and check-linux.sh.
# Sourced, never executed directly.

PRC_LLIB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PRC_ROOT="$(cd "$PRC_LLIB/.." && pwd)"
PRC_IOC="${PRC_IOC_DIR:-$PRC_ROOT/ioc}"

# Obfuscation tells used to qualify a "content after module end" finding.
# Only these file types can carry the payload. Without this filter a recursive
# grep over an IDE extension tree reads gigabytes and takes minutes.
PRC_CODE_INCLUDES='--include=*.js --include=*.mjs --include=*.cjs --include=*.ts --include=*.json --include=*.map --include=*.sh --include=*.bat --include=*.ps1'

PRC_TAIL_TELL='eval\(|new Function\(|Buffer\.from\(|child_process|atob\(|fromCharCode|\\x[0-9a-fA-F]{2}\\x[0-9a-fA-F]{2}|require\([^)]*(child_process|https?|net|dns)'

HITS=0
REVIEW=0
APPLY=0
QDIR=""
REPORT=""

# Everything printed can contain bytes from a file an attacker controls: a path,
# a matched line, a font's magic bytes. Control characters are stripped so a
# crafted filename cannot drive the operator's terminal with escape sequences,
# and so the report file stays greppable.
prc_clean() { LC_ALL=C tr -d '\000-\010\013\014\016-\037\177'; }
say()  { local t; t="$(printf '%s' "$*" | prc_clean)"; printf '%s\n' "$t" | tee -a "$REPORT" >/dev/null; printf '%s\n' "$t"; }
# Long inventories go to the report file only. On the console they become one
# summary line, because a wall of paths nobody reads is worse than a count.
note() { printf '%s\n' "$*" | prc_clean >> "$REPORT"; }
hdr()  { say ""; say "== $* =="; }
bad()  { HITS=$((HITS+1));     say "  [HIT]    $*"; }
warn() { REVIEW=$((REVIEW+1)); say "  [review] $*"; }
ok()   { say "  [ok]     $*"; }
# [info] is inventory or hardening advice, not evidence. It does not count
# towards the review total, because "8 items need a human look" is not true when
# six of them are "you own an SSH key".
info() { say "  [info]   $*"; }

# ---------------------------------------------------------------------------
# Indicator loading. Files, not inline strings, so every script agrees.
# ---------------------------------------------------------------------------
prc_local_load_iocs() {
  STRONG_ARGS=(); WEAK_ARGS=(); PKG_ARGS=()
  local line
  while IFS= read -r line; do STRONG_ARGS+=(-e "$line"); done \
    < <(sed -e '/^#/d' -e '/^$/d' "$PRC_IOC/strong.txt" "$PRC_IOC/bad-packages.txt")
  while IFS= read -r line; do WEAK_ARGS+=(-e "$line"); done \
    < <(sed -e '/^#/d' -e '/^$/d' "$PRC_IOC/weak.txt")
  while IFS= read -r line; do PKG_ARGS+=(-e "$line"); done \
    < <(sed -e '/^#/d' -e '/^$/d' "$PRC_IOC/bad-packages.txt")
  NET_ARGS=()
  while IFS= read -r line; do NET_ARGS+=(-e "$line"); done \
    < <(sed -e '/^#/d' -e '/^$/d' "$PRC_IOC/network.txt")
  IMPLANT_ARGS=()
  while IFS= read -r line; do IMPLANT_ARGS+=(-e "$line"); done \
    < <(sed -e '/^#/d' -e '/^$/d' "$PRC_IOC/implant-names.txt")
  [[ ${#STRONG_ARGS[@]} -gt 0 ]] || { echo "indicator set is empty" >&2; exit 2; }
}

has_strong() { grep -qaF "${STRONG_ARGS[@]}" "$1" 2>/dev/null; }
has_weak()   { grep -qaF "${WEAK_ARGS[@]}"   "$1" 2>/dev/null; }

# ---------------------------------------------------------------------------
# Quarantine. Moves, never deletes. Dry run unless --apply was given.
# ---------------------------------------------------------------------------
quarantine() {
  local src="$1" reason="$2" dest
  if [[ $APPLY -eq 0 ]]; then
    say "           would quarantine: $src"
    return 0
  fi
  dest="$QDIR/files/${src#/}"
  mkdir -p "$(dirname "$dest")" 2>/dev/null || { say "           QUARANTINE FAILED (mkdir): $src"; return 1; }
  if mv "$src" "$dest" 2>/dev/null; then
    printf '%s\t%s\t%s\n' "$src" "$dest" "$reason" >> "$QDIR/manifest.tsv"
    say "           quarantined -> $dest"
  else
    say "           QUARANTINE FAILED (mv, check permissions): $src"
    return 1
  fi
}

quarantine_init() {
  [[ $APPLY -eq 0 ]] && return 0
  mkdir -p "$QDIR/files" || { echo "cannot create $QDIR" >&2; exit 2; }
  printf 'original_path\tquarantined_path\treason\n' > "$QDIR/manifest.tsv"
  cat > "$QDIR/RESTORE.txt" <<'RES'
Nothing here was deleted. To put a file back:

  while IFS=$'\t' read -r orig dest reason; do
    [ "$orig" = "original_path" ] && continue
    mkdir -p "$(dirname "$orig")" && mv "$dest" "$orig"
  done < manifest.tsv

Keep this directory until the incident is closed. It is evidence.
RES
}

# ---------------------------------------------------------------------------
# Checks shared by macOS and Linux
# ---------------------------------------------------------------------------

# shellcheck disable=SC2086  # PRC_CODE_INCLUDES must word-split into separate --include flags
prc_sha256() {
  if   command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" 2>/dev/null | awk '{print $1}'
  elif command -v shasum    >/dev/null 2>&1; then shasum -a 256 "$1" 2>/dev/null | awk '{print $1}'
  fi
}

# Match a file against the known second-stage hashes, whatever it is called.
prc_hash_verdict() {
  local f="$1" h label
  h="$(prc_sha256 "$f")"
  [[ -z "$h" ]] && return 1
  label="$(awk -v h="$h" '$1==h {$1=""; sub(/^ +/,""); print; exit}' "$PRC_IOC/hashes.txt")"
  [[ -n "$label" ]] || return 1
  printf '%s' "$label"
}

# The second stage is a Node.js Single Executable Application: a native binary
# with V8 and the payload linked in. It sets its own process title to look like
# a system service, so the process table is as important as the filesystem here.
check_implants() {
  hdr "Second-stage implant"
  local line path label found=0 uid
  uid="$(id -u 2>/dev/null || echo 0)"

  while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    case "$line" in '%'*) continue ;; esac   # Windows path, check-windows.ps1 handles it
    path="${line/#\~/$HOME}"
    [[ -e "$path" ]] || continue
    found=1
    if [[ -f "$path" ]] && label="$(prc_hash_verdict "$path")"; then
      bad "implant binary confirmed by hash ($label): $path"
    else
      bad "implant artifact present: $path"
    fi
    case "$path" in
      *"/LaunchAgents/"*)
        say "           stop it first: launchctl bootout gui/${uid} '$path' 2>/dev/null || launchctl unload '$path'" ;;
      *"/systemd/user/"*)
        say "           stop it first: systemctl --user disable --now '$(basename "$path")'"
        say "           and: loginctl disable-linger \"$(whoami)\"" ;;
      *"/autostart/"*)
        say "           it will not start again once this file is quarantined" ;;
    esac
    quarantine "$path" "second-stage-implant"
  done < <(sed -e '/^#/d' -e '/^$/d' "$PRC_IOC/implant-paths.txt")

  # A renamed binary still hashes the same.
  local root f
  for root in "$@"; do
    [[ -d "$root" ]] || continue
    while read -r f; do
      [[ -z "$f" ]] && continue
      if label="$(prc_hash_verdict "$f")"; then
        found=1
        bad "file matches a known implant hash ($label): $f"
        quarantine "$f" "second-stage-implant"
      fi
    done < <(find "$root" -type f -size +10M -size -300M \
               -not -path '*/node_modules/*' -not -path '*/.git/*' 2>/dev/null | head -200)
  done

  # The process title is set by the implant, so a match here is a finding even
  # with nothing on disk.
  #
  # Match the process NAME exactly, never the full command line. Anything that
  # merely mentions the implant - this scanner, an administrator grepping for
  # it, an editor with the file open - would otherwise be reported as a running
  # implant. That is the same self-signature collision that makes a grep-based
  # scanner flag its own detection rules.
  local procs re
  re="$(sed -e '/^#/d' -e '/^$/d' "$PRC_IOC/implant-names.txt" \
        | sed 's/[].[^$*\\]/\\&/g' | paste -sd'|' - )"
  procs="$(ps ax -o pid=,comm= 2>/dev/null \
           | awk -v re="^(${re})$" -v self="$$" '{n=$2; sub(/.*\//,"",n); if (n ~ re && $1 != self) print}' \
           | cut -c1-200 | head -10)"
  if [[ -n "$procs" ]]; then
    found=1
    printf '%s\n' "$procs" | sed 's/^/    /' | tee -a "$REPORT" >/dev/null
    printf '%s\n' "$procs" | sed 's/^/    /'
    bad "an implant process is running now. Kill it before anything else:"
    printf '%s\n' "$procs" | awk '{print "           kill -9 " $1}' | tee -a "$REPORT"
  fi

  [[ $found -eq 0 ]] && ok "no second-stage implant found"
}

check_extensions() {   # $@ = extension directories
  hdr "IDE extensions"
  local d f found=0
  for d in "$@"; do
    [[ -d "$d" ]] || continue
    found=1
    local seen_ext=""
    while read -r f; do
      [[ -z "$f" ]] && continue
      local extdir="$f"
      # walk up to the extension's own directory under $d
      while [[ "$(dirname "$extdir")" != "$d" && "$extdir" != "/" ]]; do extdir="$(dirname "$extdir")"; done
      case "$seen_ext" in *"|$extdir|"*) continue ;; esac
      seen_ext="${seen_ext}|${extdir}|"
      bad "extension contains an indicator: $extdir"
      quarantine "$extdir" "ide-extension"
    done < <(grep -RlaF $PRC_CODE_INCLUDES "${STRONG_ARGS[@]}" "$d" 2>/dev/null | head -40)
    # The generic weak list is useless here. A bundled extension legitimately
    # contains "folderOpen" (it is a codicon name) and "windowsHide" (a standard
    # child_process option), so matching those produces pages of noise. Only
    # actual campaign infrastructure is worth a human's attention in an
    # extension bundle, and the message names what matched.
    while read -r f; do
      [[ -z "$f" ]] && continue
      local term
      term="$(grep -haoF "${NET_ARGS[@]}" "$f" 2>/dev/null | sort -u | tr '\n' ' ' | cut -c1-60)"
      warn "extension references campaign infrastructure (${term%% }): $f"
    done < <(grep -RlaF $PRC_CODE_INCLUDES "${NET_ARGS[@]}" "$d" 2>/dev/null | head -20)
  done
  [[ $found -eq 0 ]] && ok "no IDE extension directories found"
  local recent=0
  for d in "$@"; do
    [[ -d "$d" ]] || continue
    recent=$(( recent + $(find "$d" -maxdepth 1 -mindepth 1 -type d -mtime -60 2>/dev/null | grep -c .) ))
    find "$d" -maxdepth 1 -mindepth 1 -type d -mtime -60 2>/dev/null | sed 's|.*/|    |' >> "$REPORT"
  done
  [[ $recent -gt 0 ]] && info "$recent extensions installed or updated in the last 60 days. Names are in the report; check any you did not install yourself."
  return 0
}

check_tasks_json() {   # $@ = code roots
  hdr "Workspace tasks that run on folder open"
  local root f seen=0 flagged=0
  for root in "$@"; do
    [[ -d "$root" ]] || continue
    while read -r f; do
      [[ -z "$f" ]] && continue
      seen=$((seen+1))
      grep -qF 'folderOpen' "$f" 2>/dev/null || continue
      flagged=$((flagged+1))
      if has_strong "$f"; then
        bad "tasks.json runs on folder open AND contains an indicator: $f"
        quarantine "$f" "malicious-tasks-json"
      else
        warn "tasks.json runs on folder open, verify the command by hand: $f"
      fi
    done < <(find "$root" -name 'tasks.json' -path '*/.vscode/*' -not -path '*/node_modules/*' 2>/dev/null | head -200)
  done
  if [[ $flagged -eq 0 ]]; then
    if [[ $seen -eq 0 ]]; then ok "no .vscode/tasks.json found under the scanned paths"
    else ok "$seen .vscode/tasks.json checked, none run on folder open"; fi
  fi
}

check_configs() {      # $@ = code roots
  hdr "Build configs with code after the module end"
  local root f total endln seen=0 flagged=0
  for root in "$@"; do
    [[ -d "$root" ]] || continue
    while read -r f; do
      [[ -z "$f" ]] && continue
      seen=$((seen+1))
      if has_strong "$f"; then
        flagged=$((flagged+1))
        bad "config file contains an indicator: $f"
        say  "           do not edit this file. Delete the clone and re-clone after the remote is clean."
        continue
      fi
      total=$(grep -c '' "$f" 2>/dev/null || echo 0)
      endln=$(grep -n -E '^(export default|module\.exports)' "$f" 2>/dev/null | tail -1 | cut -d: -f1)
      if [[ -n "$endln" && "$total" -gt $(( endln + 15 )) ]]; then
        # Normal in flat configs. Only a signal if the remainder looks like a payload.
        if tail -n +$(( endln + 1 )) "$f" 2>/dev/null | grep -qE "$PRC_TAIL_TELL" \
           || tail -n +$(( endln + 1 )) "$f" 2>/dev/null | awk 'length($0) > 500 {found=1} END{exit !found}'; then
          flagged=$((flagged+1))
          warn "content after module end that looks like a payload ($total lines, module ends at $endln): $f"
        fi
      fi
      if awk 'length($0) > 4000 {found=1} END{exit !found}' "$f" 2>/dev/null; then
        flagged=$((flagged+1))
        warn "line longer than 4000 characters, an obfuscation tell: $f"
      fi
    done < <(find "$root" -not -path '*/node_modules/*' -not -path '*/.git/*' \
               \( -name 'postcss.config.*' -o -name 'tailwind.config.*' \
                  -o -name 'eslint.config.*' -o -name 'vite.config.*' \
                  -o -name 'next.config.*'  -o -name 'rollup.config.*' \
                  -o -name 'webpack.config.*' -o -name 'babel.config.*' \
                  -o -name 'gridsome.config.*' -o -name 'vue.config.*' -o -name 'truffle.js' \) 2>/dev/null | head -500)
  done
  [[ $flagged -eq 0 ]] && ok "$seen build config files checked, nothing appended after the module end"
}

check_fonts() {        # $@ = code roots
  hdr "Font files that are not fonts"
  local root f magic seen=0 flagged=0
  for root in "$@"; do
    [[ -d "$root" ]] || continue
    while read -r f; do
      [[ -z "$f" ]] && continue
      [[ -s "$f" ]] || continue          # empty file cannot carry a payload
      seen=$((seen+1))
      magic="$(head -c 4 "$f" 2>/dev/null)"
      case "$magic" in
        wOFF|wOF2) ;;
        vers) ;;                          # git-lfs pointer, not the font itself
        *) local hex
           hex="$(head -c 4 "$f" 2>/dev/null | od -An -tx1 | tr -s ' ' | sed 's/^ *//;s/ *$//')"
           flagged=$((flagged+1))
           bad "font file is not a font (first bytes: $hex): $f"
           quarantine "$f" "font-masquerade" ;;
      esac
    done < <(find "$root" -not -path '*/node_modules/*' -not -path '*/.git/*' \
               -not -path '*/__MACOSX/*' -not -name '._*' \
               \( -name '*.woff' -o -name '*.woff2' \) 2>/dev/null | head -600)
  done
  [[ $flagged -eq 0 ]] && ok "$seen font files checked, all are real fonts"
}

check_propagation() {  # $1 = home
  hdr "Propagation artifact temp_auto_push.bat"
  local f found=0
  while read -r f; do
    [[ -z "$f" ]] && continue
    found=1
    bad "propagation script present: $f"
    quarantine "$f" "propagation-script"
  done < <(find "$1" \( -path '*/Library/*' -o -path '*/.Trash/*' -o -path '*/.cache/*' \) -prune \
             -o -name 'temp_auto_push.bat' -print 2>/dev/null | head -20)
  [[ $found -eq 0 ]] && ok "temp_auto_push.bat not found"
}

check_packages() {     # $@ = code roots
  hdr "Known-bad packages"
  local root f found=0
  for root in "$@"; do
    [[ -d "$root" ]] || continue
    while read -r f; do
      [[ -z "$f" ]] && continue
      if grep -qaF "${PKG_ARGS[@]}" "$f" 2>/dev/null; then
        found=1
        bad "known-bad package referenced: $f"
        say  "           remove the dependency, delete node_modules and the lockfile entry, reinstall."
      fi
    done < <(find "$root" -not -path '*/node_modules/*' \
               \( -name 'package.json' -o -name 'package-lock.json' \
                  -o -name 'pnpm-lock.yaml' -o -name 'yarn.lock' \) 2>/dev/null | head -600)
  done
  [[ $found -eq 0 ]] && ok "no known-bad package names in manifests or lockfiles"
}

check_shell_rc() {     # $@ = rc files
  hdr "Shell startup files"
  local f
  for f in "$@"; do
    [[ -f "$f" ]] || continue
    if has_strong "$f"; then
      bad "shell startup file contains an indicator: $f"
      say  "           edit it by hand and remove the line. This file is never quarantined."
    elif grep -qE '(curl|wget).*\|[[:space:]]*(bash|sh|node)' "$f" 2>/dev/null; then
      bad "shell startup file pipes a download into an interpreter: $f"
      say  "           edit it by hand and remove the line."
    elif awk 'length($0) > 2000 {found=1} END{exit !found}' "$f" 2>/dev/null; then
      warn "shell startup file has a very long line: $f"
    else
      ok "clean: $f"
    fi
  done
}

check_git() {          # $@ = code roots
  hdr "Git configuration and hooks"
  local hp h root
  hp="$(git config --global core.hooksPath 2>/dev/null || true)"
  if [[ -n "$hp" ]]; then
    warn "global core.hooksPath is set to: $hp"
  else
    ok "no global core.hooksPath"
  fi
  git config --global --list 2>/dev/null \
    | grep -Ei '(url\..*insteadof|http\..*proxy|credential\.helper)' | sed 's/^/    /' | tee -a "$REPORT"
  for root in "$@"; do
    [[ -d "$root" ]] || continue
    while read -r h; do
      [[ -z "$h" ]] && continue
      if has_strong "$h"; then
        bad "git hook contains an indicator: $h"
        quarantine "$h" "git-hook"
      else
        warn "active git hook, verify by hand: $h"
      fi
    done < <(find "$root" -path '*/.git/hooks/*' -type f ! -name '*.sample' -perm -u+x 2>/dev/null | head -100)
  done
}

check_npm() {
  hdr "npm configuration"
  if [[ -f "$HOME/.npmrc" ]]; then
    say "  ~/.npmrc, tokens redacted:"
    sed -E 's/(_authToken|_auth|_password)=.*/\1=<REDACTED-ROTATE-THIS>/' "$HOME/.npmrc" | sed 's/^/    /' | tee -a "$REPORT"
    grep -q '_authToken' "$HOME/.npmrc" 2>/dev/null && \
      info "an npm auth token is stored on disk. Rotate it regardless of this scan."
    if grep -qE '^registry=' "$HOME/.npmrc" 2>/dev/null && \
       ! grep -E '^registry=' "$HOME/.npmrc" | grep -q 'registry.npmjs.org'; then
      bad "a non-default npm registry is configured"
    fi
  else
    ok "no ~/.npmrc"
  fi
  if command -v npm >/dev/null 2>&1; then
    local ign; ign="$(npm config get ignore-scripts 2>/dev/null || echo unknown)"
    if [[ "$ign" == "true" ]]; then ok "npm ignore-scripts is on"
    else info "npm ignore-scripts is '$ign'. Hardening, not a finding: npm config set ignore-scripts true"; fi
  fi
}

# Inventory, not findings. Owning an SSH key is not suspicious, and a .pub file
# is a public key, not a credential. Listed so you know what to rotate IF
# something else was a confirmed hit, which is why these are [info].
check_credentials() {  # $@ = code roots
  hdr "Credential surface on this machine"
  local f n envcount=0 root found=0
  for f in "$HOME"/.ssh/id_* "$HOME/.aws/credentials" "$HOME/.config/gcloud/credentials.db" \
           "$HOME/.docker/config.json" "$HOME/.kube/config" "$HOME/.netrc"; do
    [[ -e "$f" ]] || continue
    case "$f" in *.pub) continue ;; esac    # a public key is not a credential
    found=$((found+1))
    note "  credential material: $f"
  done
  for root in "$@"; do
    [[ -d "$root" ]] || continue
    n=$(find "$root" -name '.env*' -not -path '*/node_modules/*' 2>/dev/null | grep -c .)
    envcount=$(( envcount + n ))
  done
  if [[ $found -gt 0 || $envcount -gt 0 ]]; then
    info "$found private key or credential files, and $envcount .env files, under the scanned paths."
    info "None of this is a finding. It is the list to rotate if anything else was a HIT."
    info "Full paths are in the report file."
  else
    ok "no credential files found under the scanned paths"
  fi
}

check_processes() {
  hdr "Resident interpreters running inline code"
  local out
  # shellcheck disable=SC2009  # pgrep cannot return the full command line portably
  out="$(ps ax -o pid=,command= 2>/dev/null | grep -E '(node|python[0-9.]*)[[:space:]]+-(e|c)[[:space:]]' \
         | grep -v grep | head -20)"
  if [[ -n "$out" ]]; then
    printf '%s\n' "$out" | sed 's/^/    /' | prc_clean >> "$REPORT"
    printf '%s\n' "$out" | cut -c1-110 | sed -e 's/$/ .../' -e 's/^/    /'
    if printf '%s' "$out" | grep -qaF "${IMPLANT_ARGS[@]}"; then
      bad "an interpreter is running implant code right now"
    else
      warn "an interpreter is running code passed on the command line. Read each one."
      info "editors and coding agents do this legitimately. Full command lines are in the report."
    fi
  else
    ok "no interpreter running inline code right now"
  fi
}

# check_network <lines of connection output>
check_network() {
  local out="$1"
  if [[ -z "$out" ]]; then
    ok "no established node or Electron TCP connections right now"
    return
  fi
  local n; n="$(printf '%s\n' "$out" | grep -c .)"
  printf '%s\n' "$out" | sed 's/^/    /' | prc_clean >> "$REPORT"
  if printf '%s\n' "$out" | grep -qF "${NET_ARGS[@]}" 2>/dev/null; then
    printf '%s\n' "$out" | sed 's/^/    /'
    bad "live connection to known campaign infrastructure"
  else
    ok "$n established connections from editors and node, none to known campaign infrastructure"
    info "the connection list is in the report file"
  fi
}

# ---------------------------------------------------------------------------
verdict() {
  hdr "RESULT"
  say "  confirmed indicator hits : $HITS"
  say "  items needing a human    : $REVIEW"
  say "  full report              : $REPORT"
  [[ $APPLY -eq 1 ]] && say "  quarantine               : $QDIR"
  say ""
  if [[ $HITS -gt 0 ]]; then
    say "VERDICT: COMPROMISED."
    say "  Quarantining artifacts does not make this machine trustworthy again. The"
    say "  payload is a remote access trojan and an infostealer, so assume every"
    say "  credential reachable from this user account has been taken."
    say "  1. Disconnect from the network."
    say "  2. Rotate every credential in the section above, from a different machine."
    say "  3. Rebuild from a clean OS install. Do not restore a backup taken after"
    say "     the infection date."
    say "  4. Delete every local clone. Re-clone only after the remote is verified clean."
    return 2
  elif [[ $REVIEW -gt 0 ]]; then
    say "VERDICT: no confirmed indicator."
    say ""
    if [[ $REVIEW -eq 1 ]]; then
      say "  One thing needs your eyes: the [review] line above."
    else
      say "  $REVIEW things need your eyes: the [review] lines above."
    fi
    say "  The [info] lines are inventory and hardening advice, not findings."
    say ""
    say "  A clean result proves the current indicator set is absent. It does not"
    say "  prove an older or rotated variant was never here. Rotate your GitHub"
    say "  tokens, SSH keys and cloud keys anyway."
    return 1
  else
    say "VERDICT: clean against the current indicator set."
    say "  Rotate credentials anyway. Yours may have been taken from a different"
    say "  machine or from a shared secret store."
    return 0
  fi
}
