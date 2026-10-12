# 0043. Package in layouts the binary already searches, and bump by pull request

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-13 |

## Context

2.0.0 shipped as `.tar.gz` archives, a Homebrew formula and a Scoop manifest,
the last two bumped by hand. Debian and Ubuntu users had no package, winget
installs only from an installer or a zip, and every hand bump is a chance to
paste the wrong checksum into a security tool's package.

The binary finds its indicators by resolving its own real path and walking up
from it, looking for `ioc/` and for `share/polinrider/ioc` at each level
(`default_ioc()` in `src/cli.rs`). Any package has to put `ioc/` where that
walk reaches, or change the engine.

## Decision

- **No engine change.** The `.deb` installs `/usr/bin/polinrider` and
  `/usr/share/polinrider/ioc`, which the walk reaches at `/usr`, the same
  layout as the Homebrew formula under its prefix.
- **The Windows zip is flat**: `polinrider.exe` and `ioc/` at its top level.
  winget's portable install folder is then the exe's own folder, so `ioc/` is
  beside it whether winget links the command or adds the folder to the PATH.
  Scoop moves to the same zip, which it unpacks without 7-Zip.
- **The `.deb` is built with `dpkg-deb` alone** (`packaging/deb.sh`), with
  `Depends` read from the binary by `objdump`, and is installed and run on the
  disposable runner in both `release.yml` and `beta-build.yml`.
- **The tap and bucket are bumped by pull request**, from the release's
  `SHA256SUMS`, by a job holding a fine-grained token scoped to those two
  repositories. The commit is made through GitHub's API so GitHub signs it, and
  the job checks the signature before it opens the pull request.
- **winget is generated, not submitted.** The manifests are a run artifact for
  the maintainer to submit to `microsoft/winget-pkgs` by hand.

## Consequences

Packages cannot drift from the release: every checksum comes from the file the
attestation covers, and the maintainer still merges each bump.

What it costs:

- **The zip and the tarball differ in shape.** The tarball has a top folder;
  the zip does not, so unpacking the zip "here" spills four entries into the
  current folder. That is the price of winget finding `ioc/`.
- **winget depends on link resolution.** If a future winget changes how it
  exposes a portable command (it already turned aliases into hard links in
  1.30), the binary may stop finding `ioc/` from the PATH. `--version` says
  `NOT USABLE` when that happens, and `--ioc` is the workaround.
- **The `.deb` Depends on the build machine's glibc.** Building on the newest
  Ubuntu runner raises the floor; a user on an older release gets a clear
  dependency error rather than a binary that fails to start, but gets no
  package either.
- **A token that can write to two other repositories now exists.** It is
  scoped to those two, used by one job after a protected-tag release, and its
  output still needs a merge. When it expires the bumps stop quietly.
- **Commits in the tap and bucket are signed by GitHub, not by the
  maintainer's key.** They show as Verified, authored by the token's owner,
  like a web edit. Someone who requires the maintainer's own signature on
  every commit there will not get it.

## Related

- ADR-0027, development and testing happen in a container: the packages are
  installed and run only on disposable runners, never on a maintainer's
  machine.
