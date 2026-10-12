#!/usr/bin/env bash
# deb.sh - build the Debian package for one Linux binary, with dpkg-deb alone.
#
#   packaging/deb.sh BINARY VERSION ARCH OUT.deb
#
#   BINARY   the built polinrider for that architecture
#   VERSION  the version without the v, 2.0.1
#   ARCH     amd64 or arm64
#   OUT.deb  where to write the package
#
# Run from the repository root: it takes ioc/, LICENSE and DISCLAIMER.md from
# there. It builds a package and does nothing else; it never runs BINARY.
#
# The layout is one the binary already searches, so the engine needs no change
# for it. default_ioc() in src/cli.rs resolves the binary's real path, then
# walks up from it looking for ioc/ and for share/polinrider/ioc. From
# /usr/bin/polinrider that walk reaches /usr and finds /usr/share/polinrider/ioc,
# which is also the layout the Homebrew formula uses under its prefix.
#
#   /usr/bin/polinrider
#   /usr/share/polinrider/ioc/
#   /usr/share/doc/polinrider-cleaner/copyright        the licence
#   /usr/share/doc/polinrider-cleaner/DISCLAIMER.md

set -euo pipefail

die() { echo "deb.sh: $*" >&2; exit 1; }

[[ $# -eq 4 ]] || die "usage: packaging/deb.sh BINARY VERSION ARCH OUT.deb"
bin=$1 version=$2 arch=$3 out=$4

case "$arch" in amd64 | arm64) ;; *) die "ARCH must be amd64 or arm64, not $arch" ;; esac
[[ "$version" =~ ^[0-9][0-9A-Za-z.+~-]*$ ]] || die "not a Debian version: $version"
[[ -f "$bin" ]] || die "no binary at $bin"
[[ -f ioc/strong.txt && -f LICENSE && -f DISCLAIMER.md ]] \
  || die "run from the repository root (ioc/, LICENSE and DISCLAIMER.md)"
command -v dpkg-deb >/dev/null || die "dpkg-deb is needed"
command -v objdump >/dev/null || die "objdump is needed to read the binary's library needs"

# Depends, read from the binary rather than written down, so a build against a
# newer glibc cannot quietly install on a system that cannot run it. A Rust
# binary links libc and, for unwinding, libgcc_s. Anything else is unexpected
# and stops the build rather than shipping a package with a missing dependency.
glibc=$(objdump -T "$bin" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/^GLIBC_//' | sort -uV | tail -1)
[[ -n "$glibc" ]] || die "could not read the glibc version $bin needs"
depends="libc6 (>= ${glibc})"
while read -r lib; do
  case "$lib" in
    libc.so.6 | libm.so.6 | libpthread.so.0 | libdl.so.2 | librt.so.1 | ld-linux-*.so.*) ;;
    libgcc_s.so.1) depends="${depends}, libgcc-s1" ;;
    *) die "$bin needs $lib, which this script does not know the package for" ;;
  esac
done < <(objdump -p "$bin" | awk '$1 == "NEEDED" { print $2 }')

root=$(mktemp -d)
trap 'rm -rf "$root"' EXIT
doc="$root/usr/share/doc/polinrider-cleaner"

install -d -m 0755 "$root/DEBIAN" "$root/usr/bin" "$root/usr/share/polinrider" "$doc"
install -m 0755 "$bin" "$root/usr/bin/polinrider"
cp -R ioc "$root/usr/share/polinrider/ioc"
install -m 0644 LICENSE "$doc/copyright"
install -m 0644 DISCLAIMER.md "$doc/DISCLAIMER.md"

# Modes as policy has them, whatever the checkout's umask left behind. The
# package root too: mktemp makes it 0700, and that mode is recorded as ./
chmod 0755 "$root"
find "$root/usr" -type d -exec chmod 0755 {} +
find "$root/usr" -type f -exec chmod 0644 {} +
chmod 0755 "$root/usr/bin/polinrider"

(cd "$root" && find usr -type f -print0 | sort -z | xargs -0 md5sum) > "$root/DEBIAN/md5sums"
chmod 0644 "$root/DEBIAN/md5sums"

size=$(du -sk "$root/usr" | cut -f1)
cat > "$root/DEBIAN/control" <<EOF
Package: polinrider-cleaner
Version: ${version}
Architecture: ${arch}
Maintainer: Mandeep Singh <2369397+meSingh@users.noreply.github.com>
Installed-Size: ${size}
Depends: ${depends}
Recommends: git
Suggests: gh
Section: utils
Priority: optional
Homepage: https://github.com/meSingh/polinrider-cleaner
Description: Detect and clean up after the PolinRider supply-chain campaign
 Checks this machine, folders of code and GitHub accounts and organizations
 for the PolinRider campaign, and cleans up what it finds. Read-only unless
 you pass --apply or type yes in the guided flow. Start with: polinrider
 .
 The indicator set is installed in /usr/share/polinrider/ioc, where the
 binary looks for it.
EOF
chmod 0644 "$root/DEBIAN/control"

# xz rather than the zstd newer Ubuntu defaults to, which dpkg on Debian 11
# and older cannot read.
dpkg-deb --root-owner-group -Zxz --build "$root" "$out" >/dev/null
echo "built $out (Depends: $depends)"
