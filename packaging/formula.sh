#!/usr/bin/env bash
# formula.sh - move the Homebrew formula to a new release.
#
#   packaging/formula.sh VERSION SHA256SUMS < polinrider-cleaner.rb > new.rb
#
# Reads the formula as it is in meSingh/homebrew-tap and changes only the
# version line, each archive url and the sha256 line under it, so anything
# else edited in the tap by hand survives. Every checksum comes from the
# release's own SHA256SUMS. Changes nothing else, fetches nothing.

set -euo pipefail

die() { echo "formula.sh: $*" >&2; exit 1; }

[[ $# -eq 2 ]] || die "usage: packaging/formula.sh VERSION SHA256SUMS < formula.rb"
version=$1 sums=$2
[[ -f "$sums" ]] || die "no SHA256SUMS at $sums"

awk -v version="$version" -v sums="$sums" '
  BEGIN {
    while ((getline line < sums) > 0) {
      split(line, f, /[ \t*]+/)
      hash[f[2]] = f[1]
    }
    base = "https://github.com/meSingh/polinrider-cleaner/releases/download/v" version "/"
  }
  /^[ \t]*version "/ {
    sub(/"[^"]*"/, "\"" version "\"")
    print; next
  }
  /^[ \t]*url "/ {
    if (!match($0, /-(macos|linux|windows)-(arm64|x86_64)\.tar\.gz"/)) {
      print "formula.sh: an archive url it does not recognise: " $0 > "/dev/stderr"
      failed = 1; exit 1
    }
    platform = substr($0, RSTART + 1, RLENGTH - 9)
    file = "polinrider-v" version "-" platform ".tar.gz"
    if (!(file in hash)) {
      print "formula.sh: " file " is not in SHA256SUMS" > "/dev/stderr"
      failed = 1; exit 1
    }
    indent = $0; sub(/url.*/, "", indent)
    print indent "url \"" base file "\""
    pending = file; urls++
    next
  }
  /^[ \t]*sha256 "/ {
    if (pending == "") {
      print "formula.sh: a sha256 line with no url above it" > "/dev/stderr"
      failed = 1; exit 1
    }
    sub(/"[^"]*"/, "\"" hash[pending] "\"")
    pending = ""; shas++
    print; next
  }
  { print }
  END {
    if (failed) exit 1
    if (urls == 0 || urls != shas) {
      print "formula.sh: expected a sha256 under every url, got " urls " urls and " shas " checksums" > "/dev/stderr"
      failed = 1; exit 1
    }
  }
'
