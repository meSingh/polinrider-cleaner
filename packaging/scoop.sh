#!/usr/bin/env bash
# scoop.sh - move the Scoop manifest to a new release.
#
#   packaging/scoop.sh VERSION SHA256SUMS < polinrider-cleaner.json > new.json
#
# Reads the manifest as it is in meSingh/scoop-bucket and changes the version,
# the download with its hash and the autoupdate url. Everything else, notes and
# description included, is kept as the bucket has it.
#
# The manifest points at the Windows .zip rather than the .tar.gz. Scoop
# unpacks a zip itself and needs 7-Zip for a tar.gz, and the zip is the archive
# winget installs too, so both package managers install the same bytes. The zip
# holds polinrider.exe and ioc/ at its top level, so there is no extract_dir.

set -euo pipefail

die() { echo "scoop.sh: $*" >&2; exit 1; }

[[ $# -eq 2 ]] || die "usage: packaging/scoop.sh VERSION SHA256SUMS < manifest.json"
version=$1 sums=$2
[[ -f "$sums" ]] || die "no SHA256SUMS at $sums"
command -v jq >/dev/null || die "jq is needed"

file="polinrider-v${version}-windows-x86_64.zip"
hash=$(awk -v f="$file" '$2 == f || $2 == "*" f { print $1 }' "$sums")
[[ "$hash" =~ ^[0-9a-f]{64}$ ]] || die "$file is not in SHA256SUMS"

base="https://github.com/meSingh/polinrider-cleaner/releases/download"
# $version in the autoupdate url is Scoop's placeholder, not a shell variable.
# shellcheck disable=SC2016
auto='https://github.com/meSingh/polinrider-cleaner/releases/download/v$version/polinrider-v$version-windows-x86_64.zip'

# --indent 4 is the bucket's own formatting, so the pull request shows only
# what changed.
jq --indent 4 \
  --arg version "$version" \
  --arg url "${base}/v${version}/${file}" \
  --arg hash "$hash" \
  --arg auto "$auto" '
    .version = $version
    | .architecture."64bit" = { url: $url, hash: $hash }
    | .autoupdate.architecture."64bit" = { url: $auto }
  '
