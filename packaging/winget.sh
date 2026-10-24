#!/usr/bin/env bash
# winget.sh - write the winget manifest set for a release.
#
#   packaging/winget.sh VERSION SHA256SUMS OUTDIR [RELEASE_DATE]
#
# Writes the three files microsoft/winget-pkgs expects (version, installer and
# defaultLocale) under OUTDIR/manifests/m/meSingh/PolinRiderCleaner/VERSION/,
# the path they take in that repository. Submitting them is done by hand; this
# writes files and nothing else.
#
# The installer is the Windows .zip, unpacked as a portable package. The zip
# holds polinrider.exe and ioc/ at its top level, so winget's install folder is
# the folder the exe is in and ioc/ is beside it. winget then puts a symlink to
# the exe on the PATH; the binary resolves that link to its real path before it
# looks for ioc/ (default_ioc() in src/cli.rs), so it finds its indicators.
#
# Do not add PortableCommandAlias or Commands. Since winget 1.30 an alias is a
# hard link, which cannot be resolved the way a symbolic link can, so whether
# ioc/ is found would depend on where winget puts it. polinrider.exe is
# already the command name.

set -euo pipefail

die() { echo "winget.sh: $*" >&2; exit 1; }

[[ $# -eq 3 || $# -eq 4 ]] || die "usage: packaging/winget.sh VERSION SHA256SUMS OUTDIR [RELEASE_DATE]"
version=$1 sums=$2 out=$3 date=${4:-$(date -u +%Y-%m-%d)}
[[ -f "$sums" ]] || die "no SHA256SUMS at $sums"
[[ "$date" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] || die "RELEASE_DATE must be YYYY-MM-DD, not $date"

id=meSingh.PolinRiderCleaner
schema=1.12.0
repo=https://github.com/meSingh/polinrider-cleaner
file="polinrider-v${version}-windows-x86_64.zip"
hash=$(awk -v f="$file" '$2 == f || $2 == "*" f { print $1 }' "$sums")
[[ "$hash" =~ ^[0-9a-f]{64}$ ]] || die "$file is not in SHA256SUMS"
hash=$(printf '%s' "$hash" | tr '[:lower:]' '[:upper:]')

dir="${out}/manifests/m/meSingh/PolinRiderCleaner/${version}"
mkdir -p "$dir"

cat > "${dir}/${id}.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.version.${schema}.schema.json

PackageIdentifier: ${id}
PackageVersion: ${version}
DefaultLocale: en-US
ManifestType: version
ManifestVersion: ${schema}
EOF

cat > "${dir}/${id}.installer.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.installer.${schema}.schema.json

PackageIdentifier: ${id}
PackageVersion: ${version}
InstallerType: zip
NestedInstallerType: portable
NestedInstallerFiles:
- RelativeFilePath: polinrider.exe
ReleaseDate: ${date}
Installers:
- Architecture: x64
  InstallerUrl: ${repo}/releases/download/v${version}/${file}
  InstallerSha256: ${hash}
ManifestType: installer
ManifestVersion: ${schema}
EOF

cat > "${dir}/${id}.locale.en-US.yaml" <<EOF
# yaml-language-server: \$schema=https://aka.ms/winget-manifest.defaultLocale.${schema}.schema.json

PackageIdentifier: ${id}
PackageVersion: ${version}
PackageLocale: en-US
Publisher: Mandeep Singh
PublisherUrl: https://github.com/meSingh
PublisherSupportUrl: ${repo}/issues
PackageName: polinrider-cleaner
PackageUrl: ${repo}
License: MIT
LicenseUrl: ${repo}/blob/v${version}/LICENSE
ShortDescription: Detect and clean up after the PolinRider supply-chain campaign
Description: |-
  Checks this machine, folders of code and GitHub accounts and organizations for the PolinRider
  campaign, and cleans up what it finds. Read-only unless you pass --apply or type yes in the
  guided flow. Start with: polinrider
Moniker: polinrider
Tags:
- security
- incident-response
- supply-chain
- malware
ReleaseNotesUrl: ${repo}/releases/tag/v${version}
ManifestType: defaultLocale
ManifestVersion: ${schema}
EOF

echo "wrote ${dir}"
