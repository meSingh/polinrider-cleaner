# 0057. Rename the repository and its packages to polinrider-scanner, from 3.0.0

| | |
|---|---|
| **Status** | Proposed |
| **Date** | 2026-11-04 |

## Context

The repository was `meSingh/polinrider-cleaner`, and the documentation site
was a project site at `mesingh.github.io/polinrider-cleaner/`. Every package
carried the same name: the Homebrew formula, the Scoop manifest, the `.deb`
and the winget package, which was submitted but never merged (ID
`meSingh.PolinRiderCleaner`). The binary has always been `polinrider`.

The maintainer has registered `polinriderscanner.com` for the site and
`polinrider.com`, which forwards to it, and has decided to rename the
repository and every package to `polinrider-scanner`.

A rename is not free, and four parts of it reach past this repository.

- **Attestations.** Every release up to 2.3.0 is attested by a certificate
  that records the repository by name: the SAN
  `https://github.com/meSingh/polinrider-cleaner/.github/workflows/release.yml@refs/tags/vX.Y.Z`
  and the source repository URI `https://github.com/meSingh/polinrider-cleaner`.
  The certificate also records the repository ID, 1358099890, which a rename
  keeps. Given `--repo OWNER/NAME`, `gh attestation verify` requires the
  source repository URI to be `https://github.com/OWNER/NAME` and the SAN to
  start with it. So after the rename, `--repo meSingh/polinrider-scanner`
  cannot verify a release before the rename. Whether the old name still works
  through GitHub's redirect is not known until it is tried.
- **The site.** GitHub redirects everything about a renamed repository except
  the URL of its project site, so every link to
  `mesingh.github.io/polinrider-cleaner/` returns 404 after the rename. That
  includes the ADRs and indicator reviews, which are never edited.
- **Packages.** Each package manager handles a new name differently, and
  someone who installed the old name has to arrive at the new one without
  reinstalling by hand where that can be avoided.
- **The old name.** If a repository called `polinrider-cleaner` is ever
  created under this account, GitHub's redirects for the old name stop. That
  breaks every link in releases before 3.0.0 and in the commits, pull
  requests and issues the tool has opened on other people's repositories.

## Decision

- **The repository becomes `meSingh/polinrider-scanner`, and the next release
  is 3.0.0.** It is a major release because the install commands change. The
  binary, its options, its output and its exit codes do not.
- **3.0.0 is the new attestation anchor.** Every 3.x release is verified with
  `--repo meSingh/polinrider-scanner`. Releases before 3.0.0 are verified with
  whichever command is shown to work right after the rename: the old name, or
  `--owner meSingh` with a `--cert-identity-regex` that accepts both names. If
  neither works, the site gets a page that says so and shows what does. The
  result is recorded here when it is known, and this record moves to
  Accepted.
- **The site moves to `https://polinriderscanner.com/`.** Pages publishes it
  with the domain set in the repository settings, and the domain is verified
  to the account so no one else can publish on it. Both domains are on
  Cloudflare. `polinrider.com` answers every path with a 301 to the threat
  page, `https://polinriderscanner.com/threat/`, from a Cloudflare redirect
  rule, because Pages serves one domain per site. A
  new user site, `meSingh/meSingh.github.io`, holds redirect pages at
  `/polinrider-cleaner/...`, which is where the old links point.
- **Every package is renamed, each with a path from the old name:**
  - Homebrew: the formula is renamed and `formula_renames.json` maps the old
    name to the new one, so `brew upgrade` moves existing installs.
  - Scoop: a new manifest. The old one moves to the bucket's `deprecated/`
    folder, which `scoop status` reports.
  - `.deb`: the package `Conflicts:` with, `Replaces:` and `Provides:`
    `polinrider-cleaner`, so installing it removes the old one.
  - winget: the two open pull requests are closed unmerged, and the package
    is submitted as `meSingh.PolinRiderScanner`.
- **The old name is never reused.** That rule sits in `AGENTS.md` under
  "Outside this repository".

## Consequences

- People who verify an old release with the command printed beside it, which
  names `polinrider-cleaner`, may find it fails after the rename. The README,
  the release notes and the site point to the command that works, and this
  record says which one that is once it has been tried.
- The old site URLs answer with a meta refresh, not a 301. Search engines take
  that as a weaker signal, and the site starts its search history again under
  the new domain.
- A Scoop user on the old name does not get 3.0.0 by `scoop update`. Their
  `scoop status` reports the app as deprecated, and they have to uninstall it
  and install the new name. That is the cost of using Scoop's own mechanism
  rather than keeping two manifests in step.
- The Scorecard history is kept by name on scorecard.dev, so the new name
  starts from its first run after the rename.
- The rename can be undone up to the first 3.0.0 tag. After that, 3.0.0 is
  attested under the new name for good.
- `meSingh/meSingh.github.io` is a new repository that has to stay as long as
  the old links get traffic. Creating it ends the redirect GitHub kept from
  that name to `meSingh/mSingh.com_old_v5`.

## Related

- ADR-0055, the plain-Astro site, whose "Moving to a root domain" is the
  site's half of this move. Its step 4, GitHub redirecting `github.io`
  addresses to the custom domain, covers the site's address under the new
  repository name. The old `/polinrider-cleaner/` path is a different project
  path once the repository is renamed, and GitHub does not redirect project
  site URLs on a rename, which is why the redirect repository exists.
- The packaging it changes is described in `AGENTS.md`, "Cutting a release".
