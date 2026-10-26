#!/usr/bin/env bash
# scan-workspace.sh - scan a checked-out working tree, and optionally every ref in
#                     its history, for PolinRider indicators.
#
# Self-contained. Needs bash, git and standard POSIX tools, plus perl and jq
# where they are installed (both are on GitHub's hosted runners, and perl ships
# with macOS and Git for Windows). No Node, no Python, no third-party action.
# Without perl, files written in escape codes are not decoded; without jq,
# package.json and package-lock.json are matched as text. Either gap is said
# in the output, never passed over in silence.
#
# Vendor this file and the ioc/ directory into your own repository so a
# supply-chain compromise of someone else's tooling cannot compromise your scan.
#
# Usage:
#   ./scan-workspace.sh [--path DIR] [--all-refs] [--fail-on LEVEL] [--exclude RE]...
#
# Options:
#   --path DIR        directory to scan. Default: current directory
#   --all-refs        also scan every git ref, not just the working tree.
#                     Catches a payload that was committed and then reverted.
#   --fail-on LEVEL   infected (default) | review | never
#   --exclude RE      extended regex of paths to skip. Repeatable.
#   --scan-docs       also scan .md files. Skipped by default: documentation about
#                     this campaign contains the indicator strings it describes,
#                     and markdown does not execute.
#   --ioc DIR         indicator directory. Default: ioc/ beside or above this script
#
# Exit codes: 0 clean, 1 review items only (with --fail-on review), 2 infected,
#             3 could not scan. 3 is never a verdict about the code you pointed it at.

set -uo pipefail

SDIR="$(cd "$(dirname "$0")" && pwd)"
PATH_ARG="."; ALL_REFS=0; FAIL_ON="infected"; IOC_DIR=""; SCAN_DOCS=0; EXTRA_EXCLUDES=""
EXCLUDES='(^|/)\.git/|(^|/)node_modules/|(^|/)\.github/polinrider/|(^|/)ioc/|(^|/)\.github/workflows/[^/]*polinrider[^/]*\.ya?ml$'

while [[ $# -gt 0 ]]; do
  case "$1" in
    --path)     PATH_ARG="$2"; shift 2 ;;
    --all-refs) ALL_REFS=1; shift ;;
    --fail-on)  FAIL_ON="$2"; shift 2 ;;
    --exclude)  EXTRA_EXCLUDES="$EXTRA_EXCLUDES|$2"; shift 2 ;;
    --scan-docs) SCAN_DOCS=1; shift ;;
    --ioc)      IOC_DIR="$2"; shift 2 ;;
    -h|--help)  sed -n '2,30p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 3 ;;
  esac
done

# Markdown carries indicator strings whenever someone documents this campaign,
# and it cannot execute. Skipped unless asked for.
[[ $SCAN_DOCS -eq 0 ]] && EXCLUDES="$EXCLUDES|\.md$"
EXCLUDES="$EXCLUDES$EXTRA_EXCLUDES"

if [[ -z "$IOC_DIR" ]]; then
  if   [[ -f "$SDIR/ioc/strong.txt"    ]]; then IOC_DIR="$SDIR/ioc"
  elif [[ -f "$SDIR/../ioc/strong.txt" ]]; then IOC_DIR="$SDIR/../ioc"
  else echo "cannot find ioc/strong.txt. Pass --ioc DIR" >&2; exit 3; fi
fi
[[ -d "$PATH_ARG" ]] || { echo "no such directory: $PATH_ARG" >&2; exit 3; }

# The two optional helpers. The variables exist so the self-test can take
# either away and check that the gap is reported.
PERL="${POLINRIDER_PERL:-perl}"; JQ="${POLINRIDER_JQ:-jq}"
HAVE_PERL=0; command -v "$PERL" >/dev/null 2>&1 && HAVE_PERL=1
HAVE_JQ=0;   command -v "$JQ"   >/dev/null 2>&1 && HAVE_JQ=1

WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
strip() { sed -e 's/[[:space:]]*$//' -e '/^#/d' -e '/^$/d' "$@"; }
strip "$IOC_DIR/strong.txt"       > "$WORK/strong"
strip "$IOC_DIR/bad-packages.txt" > "$WORK/pkgs"
strip "$IOC_DIR/weak.txt"         > "$WORK/weak"
strip "$IOC_DIR/filenames.txt"    > "$WORK/filenames"
# Content patterns at the confirmed tier (ADR-0052). grep -E runs them
# directly; absent from an indicator set vendored before they existed.
: > "$WORK/patterns"
[[ -f "$IOC_DIR/strong-patterns.txt" ]] && strip "$IOC_DIR/strong-patterns.txt" > "$WORK/patterns"
# Names worth a look, not proof (ADR-0048). Absent from an indicator set
# vendored before it existed, which is not an error.
: > "$WORK/review-names"
[[ -f "$IOC_DIR/filenames-review.txt" ]] && strip "$IOC_DIR/filenames-review.txt" > "$WORK/review-names"
[[ -s "$WORK/strong" ]] || { echo "indicator set is empty" >&2; exit 3; }

# A campaign package name counts only as a whole name (ADR-0041): bounded on
# both sides by something that cannot be part of a package name, and not the
# second half of somebody's scope. `npm i evil-pkg` and `x/evil-pkg/` match;
# `evil-pkg-utils`, `not-evil-pkg` and `@other/evil-pkg` do not. Two patterns
# per name: one for a name that does not follow a slash, and one for a name
# after a path segment that is not a scope.
NC='A-Za-z0-9._-'
while IFS= read -r name; do
  e="$(printf '%s' "$name" | sed 's/[].[\\*^$+?(){}|]/\\&/g')"
  printf '(^|[^@/%s])%s([^%s]|$)\n' "$NC" "$e" "$NC"
  printf '(^|[^@%s])[%s]*/+%s([^%s]|$)\n' "$NC" "$NC" "$e" "$NC"
done < "$WORK/pkgs" > "$WORK/pkgre"

# Files whose package names are read rather than searched for.
MANIFEST_RE='(^|/)(package\.json|package-lock\.json|yarn\.lock|pnpm-lock\.yaml)$'
# Text that holds an escape code at all (ADR-0040). Only these are decoded.
ESC_RE='\\u[0-9A-Fa-f]{4}|\\u[{][0-9A-Fa-f]{1,6}[}]|\\x[0-9A-Fa-f]{2}'

# Obfuscation tells used to qualify a "content after module end" finding.
TAIL_TELL='eval\(|new Function\(|Buffer\.from\(|child_process|atob\(|fromCharCode|\\x[0-9a-fA-F]{2}\\x[0-9a-fA-F]{2}|require\([^)]*(child_process|https?|net|dns)'

# ---------------------------------------------------------------------------
# Helpers that read files as data
# ---------------------------------------------------------------------------

# Decodes \uXXXX (surrogate pairs joined), \u{X...} and \xNN, and matches the
# decoded copy: strong.txt as fixed strings, and outside a manifest the
# whole-name package patterns. Anything that is not a valid escape is left as
# it is. A backslash before an escape does not stop it being decoded: that is
# how a loader nested one string deep is written, and the copy is only ever
# matched, never shown. Core perl only, no modules.
#   files  STRONG PKGRE   NUL-separated paths on stdin; prints each that matches
#   batch  STRONG PKGRE   git cat-file --batch output on stdin, the path as the
#                         header's fourth field; prints each object id that matches
#   decode FILE           prints FILE decoded
# shellcheck disable=SC2016  # perl source, not shell
PL='
use strict; use warnings;
sub enc { my $c = chr($_[0]); utf8::encode($c); return $c; }
sub ok { my $v = $_[0]; return $v <= 0x10FFFF && !($v >= 0xD800 && $v <= 0xDFFF); }
sub dec {
  my $t = $_[0]; my $n = 0;
  $t =~ s{\\(?:u\{([0-9A-Fa-f]{1,6})\}|u([Dd][89ABab][0-9A-Fa-f]{2})\\u([Dd][C-Fc-f][0-9A-Fa-f]{2})|u([0-9A-Fa-f]{4})|x([0-9A-Fa-f]{2}))}{
    my $all = $&; my $v;
    if    (defined $1) { $v = hex $1; }
    elsif (defined $2) { $v = 0x10000 + ((hex($2) - 0xD800) << 10) + (hex($3) - 0xDC00); }
    elsif (defined $4) { $v = hex $4; }
    else               { $v = hex $5; }
    ok($v) ? do { $n++; enc($v) } : $all;
  }ge;
  return $n ? $t : undef;
}
sub slurp { open(my $h, "<", $_[0]) or return undef; binmode $h; local $/; my $t = <$h>; close $h; return defined $t ? $t : ""; }
my $mode = shift @ARGV;
if ($mode eq "decode") { my $t = slurp($ARGV[0]); exit 1 unless defined $t; my $d = dec($t); binmode STDOUT; print defined $d ? $d : $t; exit 0; }
my ($sf, $pf) = @ARGV;
my (@strong, @re);
{ open(my $h, "<", $sf) or die; binmode $h; while (my $l = <$h>) { chomp $l; push @strong, $l if length $l; } }
{ open(my $h, "<", $pf) or die; while (my $l = <$h>) { chomp $l; push @re, qr/$l/m if length $l; } }
sub manifest { return $_[0] =~ m{(?:^|/)(?:package\.json|package-lock\.json|yarn\.lock|pnpm-lock\.yaml)\z}; }
sub hit {
  my ($path, $t) = @_; my $d = dec($t); return 0 unless defined $d;
  for my $s (@strong) { return 1 if index($d, $s) >= 0; }
  return 0 if manifest($path);
  for my $r (@re) { return 1 if $d =~ $r; }
  return 0;
}
binmode STDIN; $| = 1;
if ($mode eq "files") {
  local $/ = "\0";
  while (my $p = <STDIN>) { chomp $p; next unless length $p; my $t = slurp($p); next unless defined $t; print "$p\n" if hit($p, $t); }
} elsif ($mode eq "batch") {
  while (defined(my $h = <STDIN>)) {
    chomp $h; my ($oid, $type, $size, $rest) = split(/ /, $h, 4);
    next unless defined $size && $size =~ /^\d+$/;
    my $buf = ""; my $got = 0;
    while ($got < $size) { my $r = read(STDIN, $buf, $size - $got, $got); last unless $r; $got += $r; }
    my $nl; read(STDIN, $nl, 1);
    print "$oid\n" if hit(defined $rest ? $rest : "", $buf);
  }
}
'

# Package names, one per line, that a package.json or package-lock.json
# installs. Mirrors src/manifest.rs: the package's own name, the four
# dependency maps (a v1 lockfile's nested entries and their requires), the
# bundled list, the real package behind an npm: alias, and v2/v3 packages keys
# after the last node_modules/. Exactly one JSON object, or an error.
JQ_NAMES='
def spec_name: . as $s | (if startswith("@") then 1 else 0 end) as $f
  | ($s[$f:] | index("@")) as $at | if $at == null then $s else $s[:$f + $at] end;
def alias_target: if type == "string" and startswith("npm:") then .[4:] as $t
  | if ($t | startswith("@")) or ($t[1:] | contains("@")) then ($t | spec_name) else empty end
  else empty end;
def depmap: if type == "object" then to_entries[] | (.key, (.value | alias_target)) else empty end;
def v1: if type == "object" then to_entries[] | (.key, (.value | if type == "object" then
  (.version | alias_target), (.requires | depmap), (.dependencies | v1) else empty end)) else empty end;
if length != 1 then error("not one document") else .[0] end
| if type != "object" then error("not an object") else . end
| . as $r
| ( ($r.name | strings),
    (["dependencies", "devDependencies", "optionalDependencies", "peerDependencies"][] as $k
      | $r[$k] | if $k == "dependencies" and type == "object" and any(.[]; type == "object")
                 then v1 else depmap end),
    (($r.bundledDependencies, $r.bundleDependencies) | arrays | .[] | strings),
    ($r.packages | if type == "object" then to_entries[]
      | ((.key | if contains("node_modules/") then (split("node_modules/") | last) else empty end),
         (.value | if type == "object" then (.name | strings),
           ((.dependencies, .devDependencies, .optionalDependencies, .peerDependencies) | depmap)
           else empty end))
      else empty end) )
| select(length > 0)
'

# Shared by the two line readers: the package a `name@range` spec names.
AWK_SPEC='
function spec_name(s,   f, r, at) {
  f = (substr(s, 1, 1) == "@") ? 1 : 0; r = substr(s, f + 1); at = index(r, "@")
  return at == 0 ? s : substr(s, 1, f + at - 1)
}
function alias_target(v,   t) {
  if (substr(v, 1, 4) != "npm:") return ""
  t = substr(v, 5)
  if (!(substr(t, 1, 1) == "@" || index(substr(t, 2), "@") > 0)) return ""
  return spec_name(t)
}
function trim(s) { sub(/^[ \t]+/, "", s); sub(/[ \t]+$/, "", s); return s }
function trimc(s, c) { while (substr(s, 1, 1) == c) s = substr(s, 2); while (length(s) && substr(s, length(s)) == c) s = substr(s, 1, length(s) - 1); return s }
'
# yarn.lock, v1 and berry: entry headers and berry resolution lines.
AWK_YARN="$AWK_SPEC"'
{ sub(/\r$/, "")
  if ($0 ~ /^[ \t]*$/ || substr($0, 1, 1) == "#") next
  if ($0 ~ /^[ \t]/) {
    l = trim($0)
    if (substr(l, 1, 11) == "resolution:") print spec_name(trimc(trim(substr(l, 12)), "\""))
    next
  }
  if (substr($0, length($0)) != ":") { bad = 1; exit }
  h = substr($0, 1, length($0) - 1)
  if (h == "__metadata") next
  n = split(h, parts, ",")
  for (i = 1; i <= n; i++) {
    s = trimc(trim(parts[i]), "\""); nm = spec_name(s)
    if (nm == "") { bad = 1; exit }
    print nm
    a = alias_target(substr(s, length(nm) + 2)); if (a != "") print a
  }
}
END { exit bad }'
# pnpm-lock.yaml, v5, v6 and v9: packages and snapshots keys, and the
# dependency maps at the top level and under importers.
AWK_PNPM="$AWK_SPEC"'
BEGIN { D["dependencies"]; D["devDependencies"]; D["optionalDependencies"]; D["peerDependencies"]; mi = -1; q = sprintf("%c", 39) }
function pkey(k,   name, slash, ver, i) {
  while (substr(k, 1, 1) == "/") k = substr(k, 2)
  i = index(k, "("); if (i) k = substr(k, 1, i - 1)
  name = spec_name(k)
  if (length(name) < length(k)) return name
  slash = 0; for (i = length(k); i > 0; i--) if (substr(k, i, 1) == "/") { slash = i; break }
  if (slash > 1) { ver = substr(k, slash + 1); if (ver ~ /^[0-9]/) return substr(k, 1, slash - 1) }
  return k
}
function ykey(c,   first, rest, end_) {
  has = 1; first = substr(c, 1, 1)
  if (first == q || first == "\"") {
    rest = substr(c, 2); end_ = index(rest, first)
    if (!end_) { has = 0; return "" }
    return substr(rest, 1, end_ - 1)
  }
  end_ = index(c, ": ")
  if (end_) return trimc(trimc(trim(substr(c, 1, end_ - 1)), q), "\"")
  if (substr(c, length(c)) == ":") return trimc(trimc(trim(substr(c, 1, length(c) - 1)), q), "\"")
  has = 0; return ""
}
/^lockfileVersion:/ { okv = 1 }
{ sub(/\r$/, ""); line = $0; c = line; sub(/^ +/, "", c)
  if (c == "" || substr(c, 1, 1) == "#") next
  ind = length(line) - length(c)
  k = ykey(c); if (!has) next
  if (ind == 0) { sec = k; mi = (k in D) ? 0 : -1; next }
  if ((sec == "packages" || sec == "snapshots") && ind == 2) out[++n] = pkey(k)
  else if (sec == "importers" && ind == 4) mi = (k in D) ? 4 : -1
  if (mi >= 0 && ind == mi + 2) out[++n] = k
}
END { if (!okv) exit 1; for (i = 1; i <= n; i++) if (out[i] != "") print out[i] }'

strip_bom() {
  if [[ "$(head -c 3 "$1" 2>/dev/null | od -An -tx1 | tr -d ' \n')" == "efbbbf" ]]; then
    tail -c +4 "$1"
  else
    cat "$1"
  fi
}

# manifest_bad BASENAME FILE: prints the campaign packages FILE installs,
# comma separated, or nothing. Returns 1 when FILE cannot be read as its
# format, 2 when it is JSON and jq is not installed.
manifest_bad() {
  local names rc
  case "$1" in
    package.json|package-lock.json)
      [[ $HAVE_JQ -eq 1 ]] || return 2
      names="$(strip_bom "$2" | "$JQ" -r -s "$JQ_NAMES" 2>/dev/null)"; rc=$? ;;
    yarn.lock)      names="$(awk "$AWK_YARN" "$2" 2>/dev/null)"; rc=$? ;;
    pnpm-lock.yaml) names="$(awk "$AWK_PNPM" "$2" 2>/dev/null)"; rc=$? ;;
    *) return 1 ;;
  esac
  [[ $rc -eq 0 ]] || return 1
  [[ -s "$WORK/pkgs" ]] || return 0
  printf '%s\n' "$names" | grep -Fx -f "$WORK/pkgs" | LC_ALL=C sort -u | paste -sd, - | sed 's/,/, /g'
  return 0
}

# Lines of $1 that are not lines of $2, without the empty-pattern-file trap.
minus() {
  if [[ -s "$2" ]]; then grep -vxF -f "$2" "$1"; else cat "$1"; fi
}

cd "$PATH_ARG" || exit 3
HITS=0; REVIEWS=0
# Findings quote file content and paths, both of which an attacker controls.
# Strip control characters so a crafted file cannot emit terminal escape
# sequences into the operator's console or into a CI log.
clean() { LC_ALL=C tr -d '\000-\010\013\014\016-\037\177'; }
note()  { printf '%s\n' "$*"; }

# The campaign ships malicious npm packages, so a skipped node_modules is a real
# gap rather than a tidy-up. Say it where it applies, not in a manual.
if [[ -d "$PATH_ARG" ]] && find "$PATH_ARG" -maxdepth 3 -type d -name node_modules -print -quit 2>/dev/null | grep -q .; then
  note "node_modules is present and NOT scanned: too large, and installed"
  note "packages are checked by name against ioc/bad-packages.txt instead."
  note "A package placed there by hand, or pulled in transitively, is not seen."
  note "To include it anyway: --exclude '' (slow)"
fi
hit()   { local m; m="$(printf '%s' "$*" | clean)"; HITS=$((HITS+1))
          printf 'INFECTED  %s\n' "$m"
          [[ "${GITHUB_ACTIONS:-}" == "true" ]] && printf '::error title=PolinRider::%s\n' "$m"; }
review(){ local m; m="$(printf '%s' "$*" | clean)"; REVIEWS=$((REVIEWS+1))
          printf 'review    %s\n' "$m"
          [[ "${GITHUB_ACTIONS:-}" == "true" ]] && printf '::warning title=PolinRider review::%s\n' "$m"; }
NOPERL_MSG="files hold escape codes and were not decoded because perl is not installed, so an indicator written in escape codes would not be seen"
UNREAD_MSG="could not be read as a manifest, so package names were matched as text"
NOJQ_MSG="could not be read as a manifest because jq is not installed, so package names were matched as text"

# ---------------------------------------------------------------------------
# Working tree
# ---------------------------------------------------------------------------
note "== working tree: $(pwd) =="

FILES="$WORK/files"; HITFILES="$WORK/hitfiles"; ESCFILES="$WORK/escfiles"
: > "$HITFILES"; : > "$ESCFILES"
# -prune stops find at the directory. Filtering afterwards with grep would
# still walk every node_modules, which is the whole cost on a real tree.
find . \( -name node_modules -o -name .git \) -prune -o -type f -print 2>/dev/null \
  | sed 's|^\./||' | grep -Ev "$EXCLUDES" > "$FILES"
note "files in scope: $(grep -c . "$FILES")"

# One grep over a whole file list, not one grep per file. On a 3,000 file tree
# the per-file loop cost 24 seconds; this costs under one. /dev/null is passed so
# grep always prefixes the filename, even when xargs hands it a single file.
scan_list() {
  local list="$1"; shift
  [[ -s "$list" ]] || return 0
  tr '\n' '\0' < "$list" | LC_ALL=C xargs -0 grep -I "$@" /dev/null 2>/dev/null
}
first_per_file() { awk -F: '!seen[$1]++'; }

# 1. indicator strings
while IFS= read -r m; do
  [[ -z "$m" ]] && continue
  printf '%s\n' "${m%%:*}" >> "$HITFILES"
  hit "$(printf '%s' "$m" | cut -c1-200)"
done < <(scan_list "$FILES" -n -F -f "$WORK/strong" | first_per_file)

# 1a. package names in manifests, read for the names they install
grep -E "$MANIFEST_RE" "$FILES" > "$WORK/manifests"
if [[ $HAVE_JQ -eq 0 ]] && grep -Eq '(^|/)package(-lock)?\.json$' "$WORK/manifests"; then
  note "jq is not installed, so package.json and package-lock.json are matched as text for package names"
fi
while IFS= read -r f; do
  [[ -z "$f" ]] && continue
  bad="$(manifest_bad "${f##*/}" "$f")"; rc=$?
  if [[ $rc -eq 0 ]]; then
    [[ -n "$bad" ]] && { printf '%s\n' "$f" >> "$HITFILES"; hit "$f: known-bad package referenced: $bad"; }
    continue
  fi
  # Unreadable: matched as 1.x matched it, and said so. Without jq only a file
  # that names a campaign package at all is worth a line.
  m="$(grep -I -n -F -f "$WORK/pkgs" /dev/null "$f" 2>/dev/null | head -1)"
  if [[ $rc -eq 2 ]]; then
    [[ -n "$m" ]] || continue
    review "$f: $NOJQ_MSG"
  else
    review "$f: $UNREAD_MSG"
  fi
  [[ -n "$m" ]] && { printf '%s\n' "$f" >> "$HITFILES"; hit "$(printf '%s' "$m" | cut -c1-200)"; }
done < "$WORK/manifests"

# 1c. campaign patterns in file contents, where a fixed string cannot say it
#     (ADR-0052). A file another pass has confirmed is not read again.
if [[ -s "$WORK/patterns" ]]; then
  minus "$FILES" "$HITFILES" > "$WORK/patscope"
  while IFS= read -r m; do
    [[ -z "$m" ]] && continue
    printf '%s\n' "${m%%:*}" >> "$HITFILES"
    hit "$(printf '%s' "$m" | cut -c1-200)"
  done < <(scan_list "$WORK/patscope" -n -E -f "$WORK/patterns" | first_per_file)
fi

# 1b. package names anywhere else, as whole names only
grep -Ev "$MANIFEST_RE" "$FILES" > "$WORK/nonmanifest"
minus "$WORK/nonmanifest" "$HITFILES" > "$WORK/pkgscope"
while IFS= read -r m; do
  [[ -z "$m" ]] && continue
  printf '%s\n' "${m%%:*}" >> "$HITFILES"
  hit "$(printf '%s' "$m" | cut -c1-200)"
done < <([[ -s "$WORK/pkgre" ]] && scan_list "$WORK/pkgscope" -n -E -f "$WORK/pkgre" | first_per_file)

# 1c. indicators written in escape codes, matched once decoded
minus "$FILES" "$HITFILES" > "$WORK/escscope"
scan_list "$WORK/escscope" -l -E "$ESC_RE" > "$WORK/esccands"
if [[ -s "$WORK/esccands" ]]; then
  if [[ $HAVE_PERL -eq 1 ]]; then
    tr '\n' '\0' < "$WORK/esccands" | "$PERL" -e "$PL" files "$WORK/strong" "$WORK/pkgre" > "$ESCFILES" 2>/dev/null
    while IFS= read -r f; do
      [[ -z "$f" ]] && continue
      printf '%s\n' "$f" >> "$HITFILES"
      hit "$f: contains an indicator once its escape codes are decoded"
    done < "$ESCFILES"
  else
    review "$(grep -c . "$WORK/esccands") $NOPERL_MSG"
  fi
fi

# 2. weak signals. Not decoded: review only, and decoding would add a line to
#    every bundle that happens to escape a common word.
while IFS= read -r m; do
  [[ -z "$m" ]] && continue
  review "$(printf '%s' "$m" | cut -c1-200)"
done < <(scan_list "$FILES" -n -F -f "$WORK/weak" | first_per_file)

# 3. indicator filenames
while IFS= read -r f; do
  [[ -n "$f" ]] && hit "$f: filename is a known propagation artifact"
done < <(grep -E -f "$WORK/filenames" "$FILES" 2>/dev/null)

# 3a. names the campaign uses that legitimate projects can have too: review
#     only, and never a path the confirmed list above already has.
if [[ -s "$WORK/review-names" ]]; then
  while IFS= read -r f; do
    [[ -n "$f" ]] && review "$f: file name the campaign also uses, verify by hand"
  done < <(grep -E -f "$WORK/review-names" "$FILES" 2>/dev/null | grep -v -E -f "$WORK/filenames" 2>/dev/null)
fi

# 4. tasks.json that runs on folder open. JSON lets folderOpen itself be
#    written in escapes, and the editor reads it the same either way.
while IFS= read -r f; do
  [[ -z "$f" ]] && continue
  if ! grep -qF 'folderOpen' "$f" 2>/dev/null; then
    [[ $HAVE_PERL -eq 1 ]] || continue
    "$PERL" -e "$PL" decode "$f" 2>/dev/null | grep -qF 'folderOpen' || continue
  fi
  if grep -qI -F -f "$WORK/strong" "$f" 2>/dev/null \
     || { [[ -s "$WORK/pkgre" ]] && LC_ALL=C grep -qI -E -f "$WORK/pkgre" "$f" 2>/dev/null; } \
     || grep -qxF -- "$f" "$ESCFILES"; then
    hit "$f: task runs on folder open and contains an indicator"
  else
    review "$f: task runs on folder open, verify the command it runs"
  fi
done < <(grep -E '(^|/)\.vscode/tasks\.json$' "$FILES" 2>/dev/null)

# 5. font masquerade
while IFS= read -r f; do
  [[ -s "$f" ]] || continue
  MAGIC="$(head -c 4 "$f" 2>/dev/null)"
  case "$MAGIC" in
    wOFF|wOF2|vers) ;;
    *) hit "$f: font file is not a font (first bytes: $(head -c 4 "$f" 2>/dev/null | od -An -tx1 | tr -s ' ' | sed 's/^ *//;s/ *$//'))" ;;
  esac
done < <(grep -Ei '\.woff2?$' "$FILES" 2>/dev/null | grep -Ev '(^|/)__MACOSX/|(^|/)\._')

# 6. build config with code after the module end
while IFS= read -r f; do
  [[ -z "$f" ]] && continue
  TOTAL="$(grep -c '' "$f" 2>/dev/null || echo 0)"
  END="$(grep -n -E '^(export default|module\.exports)' "$f" 2>/dev/null | tail -1 | cut -d: -f1)"
  if [[ -n "$END" && "$TOTAL" -gt $(( END + 15 )) ]]; then
    # Content after the module end is normal in flat configs (export default [ ... ]).
    # It is only a signal when that content also looks like a payload.
    TAIL="$(tail -n +$(( END + 1 )) "$f" 2>/dev/null)"
    if printf '%s' "$TAIL" | grep -qE "$TAIL_TELL" \
       || printf '%s' "$TAIL" | awk 'length($0) > 500 {found=1} END{exit !found}'; then
      review "$f: $TOTAL lines, module ends at $END, and the remainder looks like a payload"
    fi
  fi
  awk 'length($0) > 4000 {found=1} END{exit !found}' "$f" 2>/dev/null && \
    review "$f: line longer than 4000 characters, an obfuscation tell"
done < <(grep -E '((postcss|tailwind|eslint|vite|next|rollup|webpack|babel|gridsome|vue)\.config\.(js|mjs|cjs|ts)|truffle\.js)$' "$FILES" 2>/dev/null)

# ---------------------------------------------------------------------------
# History
# ---------------------------------------------------------------------------
# Each ref gets the same three questions as the working tree. A blob is read
# out of git, parsed or decoded once, whatever number of refs carry it: the
# answers are kept by object id.
excluded() { printf '%s' "$1" | grep -Eq "$EXCLUDES"; }
REF=""; SHOWN=0
ref_hit() {
  # At most five findings per ref, as before: a ref that carries the payload
  # in a hundred files needs reading, not a hundred lines.
  [[ $SHOWN -ge 5 ]] && return 0
  SHOWN=$(( SHOWN + 1 ))
  hit "$(printf '%s' "$1" | cut -c1-200)"
}
# The "<ref>:<path>" lines of a git grep -l, as paths, without excluded ones
# and without paths already found in this ref.
ref_paths() {
  local line p
  while IFS= read -r line; do
    p="${line#"$REF":}"
    [[ -z "$p" || "$p" == "$line" ]] && continue
    excluded "$p" && continue
    printf '%s\n' "$p"
  done | minus /dev/stdin "$WORK/refhits"
}

if [[ $ALL_REFS -eq 1 ]]; then
  if ! git rev-parse --git-dir >/dev/null 2>&1; then
    note "not a git repository, --all-refs skipped"
  else
    note ""
    note "== all refs =="
    MC="$WORK/manifest-cache"; mkdir -p "$MC"
    : > "$WORK/esc-seen"; : > "$WORK/esc-bad"; : > "$WORK/noperl"
    while IFS= read -r REF; do
      [[ -z "$REF" ]] && continue
      SHOWN=0; : > "$WORK/refhits"

      # 1. indicator strings
      while IFS= read -r line; do
        [[ -z "$line" ]] && continue
        # git grep prints "<ref>:<path>:<line>:<content>". The exclusion regex
        # matches paths, so pull the path out before testing it - testing the
        # whole line silently excludes nothing.
        rest="${line#*:}"
        gpath="${rest%%:*}"
        excluded "$gpath" && continue
        printf '%s\n' "$gpath" >> "$WORK/refhits"
        ref_hit "$line"
      done < <(git grep -I -n -F -f "$WORK/strong" "$REF" 2>/dev/null | head -200)

      # 1c. campaign patterns in contents (ADR-0052)
      if [[ -s "$WORK/patterns" ]]; then
        while IFS= read -r line; do
          [[ -z "$line" ]] && continue
          rest="${line#*:}"
          gpath="${rest%%:*}"
          excluded "$gpath" && continue
          grep -qxF -- "$gpath" "$WORK/refhits" && continue
          printf '%s\n' "$gpath" >> "$WORK/refhits"
          ref_hit "$line"
        done < <(git grep -I -n -E -f "$WORK/patterns" "$REF" 2>/dev/null | head -200)
      fi

      # 1a and 1b. package names: manifests read for names, the rest whole
      if [[ -s "$WORK/pkgs" ]]; then
        git grep -I -l -F -f "$WORK/pkgs" "$REF" 2>/dev/null | ref_paths > "$WORK/refpkg"
        : > "$WORK/refnm"
        while IFS= read -r p; do
          [[ -z "$p" ]] && continue
          if ! printf '%s' "$p" | grep -Eq "$MANIFEST_RE"; then
            printf ':(literal)%s\n' "$p" >> "$WORK/refnm"; continue
          fi
          oid="$(git rev-parse --verify -q "$REF:$p" 2>/dev/null)" || continue
          if [[ ! -f "$MC/$oid" ]]; then
            git cat-file blob "$oid" > "$WORK/blob" 2>/dev/null
            bad="$(manifest_bad "${p##*/}" "$WORK/blob")"; rc=$?
            printf '%s\n%s\n' "$rc" "$bad" > "$MC/$oid"
            # One review per blob, not one per ref that carries it.
            [[ $rc -eq 1 ]] && review "$REF:$p: $UNREAD_MSG"
            [[ $rc -eq 2 ]] && review "$REF:$p: $NOJQ_MSG"
          fi
          rc="$(sed -n 1p "$MC/$oid")"; bad="$(sed -n 2p "$MC/$oid")"
          if [[ $rc -eq 0 ]]; then
            [[ -n "$bad" ]] || continue
            printf '%s\n' "$p" >> "$WORK/refhits"
            ref_hit "$REF:$p: known-bad package referenced: $bad"
          else
            printf '%s\n' "$p" >> "$WORK/refhits"
            ref_hit "$(git grep -I -n -F -f "$WORK/pkgs" "$REF" -- ":(literal)$p" 2>/dev/null | head -1)"
          fi
        done < "$WORK/refpkg"
        if [[ -s "$WORK/refnm" && -s "$WORK/pkgre" ]]; then
          while IFS= read -r line; do
            [[ -z "$line" ]] && continue
            rest="${line#*:}"
            printf '%s\n' "${rest%%:*}" >> "$WORK/refhits"
            ref_hit "$line"
          done < <(tr '\n' '\0' < "$WORK/refnm" \
                     | LC_ALL=C xargs -0 git grep -I -n -E -f "$WORK/pkgre" "$REF" -- 2>/dev/null \
                     | awk -F: '!seen[$1 FS $2]++')
        fi
      fi

      # 1c. indicators written in escape codes. Candidates are listed by git
      #     grep, resolved to object ids, and only ids not decoded before are
      #     read, all in one cat-file batch.
      git grep -I -l -E "$ESC_RE" "$REF" 2>/dev/null | ref_paths > "$WORK/refesc"
      [[ -s "$WORK/refesc" ]] || continue
      sed "s|^|$REF:|" "$WORK/refesc" \
        | git cat-file --batch-check='%(objectname)' 2>/dev/null > "$WORK/refoids"
      paste -d' ' "$WORK/refoids" "$WORK/refesc" | grep -E '^[0-9a-f]{40,64} ' > "$WORK/refpairs"
      if [[ $HAVE_PERL -eq 0 ]]; then
        cut -d' ' -f1 "$WORK/refpairs" >> "$WORK/noperl"
        continue
      fi
      awk 'FILENAME == ARGV[1] { s[$0] = 1; next } !($1 in s) && !d[$1]++' \
        "$WORK/esc-seen" "$WORK/refpairs" > "$WORK/refnew"
      if [[ -s "$WORK/refnew" ]]; then
        cut -d' ' -f1 "$WORK/refnew" >> "$WORK/esc-seen"
        git cat-file --batch='%(objectname) %(objecttype) %(objectsize) %(rest)' < "$WORK/refnew" 2>/dev/null \
          | "$PERL" -e "$PL" batch "$WORK/strong" "$WORK/pkgre" >> "$WORK/esc-bad" 2>/dev/null
      fi
      while IFS= read -r p; do
        [[ -z "$p" ]] && continue
        ref_hit "$REF:$p: contains an indicator once its escape codes are decoded"
      done < <(awk 'FILENAME == ARGV[1] { b[$0] = 1; next }
                    ($1 in b) { p = $0; sub(/^[^ ]* /, "", p); print p }' \
                 "$WORK/esc-bad" "$WORK/refpairs")
    done < <(git for-each-ref --format='%(refname)' refs/ 2>/dev/null)
    if [[ -s "$WORK/noperl" ]]; then
      review "$(sort -u "$WORK/noperl" | grep -c .) $NOPERL_MSG (in git history)"
    fi
  fi
fi

# ---------------------------------------------------------------------------
note ""
note "========================================="
note " PolinRider workspace scan"
note "   infected findings : $HITS"
note "   review findings   : $REVIEWS"
note "========================================="
if [[ $HITS -gt 0 ]]; then
  note " A finding here means a file in this repository matches a known indicator."
  note " If this file is your own detection tooling, add its path with --exclude."
  note " Otherwise: do not merge, do not open this branch in an editor, and follow"
  note " the recovery steps in the PolinRider cleaner README."
fi
case "$FAIL_ON" in
  never)    exit 0 ;;
  review)   [[ $HITS -gt 0 ]] && exit 2; [[ $REVIEWS -gt 0 ]] && exit 1; exit 0 ;;
  infected) [[ $HITS -gt 0 ]] && exit 2; exit 0 ;;
  *) echo "unknown --fail-on value: $FAIL_ON" >&2; exit 3 ;;
esac
