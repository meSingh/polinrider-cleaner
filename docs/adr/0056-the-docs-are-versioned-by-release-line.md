# 0056. Keep a frozen copy of the docs for each release line

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-11-04 |

## Context

The docs describe one version of the tool, the one on `main`. People run
older versions: a Homebrew or Scoop install that has not been upgraded, a
binary someone downloaded during an incident, a copy of the CI scanner
vendored into a repository months ago. When a flag, a finding or a verdict
changes, the docs move on and that reader is left reading instructions for
a program they do not have. 2.3.0 is a case in point: a folder that 2.2.0
called clean can now exit 1.

The site is plain Astro (ADR-0055), so any scheme is possible. The choice
was between building every version from its git tag at deploy time, and
keeping a copy in the repository.

## Decision

- **Docs are kept per minor release line, shown as `2.3.x`.** A patch
  release changes code, not what a reader does, so it shares its line's
  docs. A new docs version is cut only when the major or minor version
  changes (2.3 to 2.4, or 2 to 3).
- **`/docs/` is always the latest release line.** Every docs page says
  which line it is for: "For polinrider 2.3.x", marked latest. The line is
  read from the newest release page, so it cannot go stale.
- **At each new minor or major release, the docs are frozen** into
  `docs-site/src/content/docs-versions/X.Y/` by
  `npm run freeze-docs -- X.Y.Z`, in the release's pull request (AGENTS.md,
  "Cutting a release", step 1b). The copy is served at `/docs/X.Y/`. For a
  patch release the script says so and does nothing. Links between docs pages inside the
  copy point at the copy; links elsewhere on the site do not change.
- **The build fails** while the newest release line has no frozen copy.
- **A frozen copy is never edited by hand.** It records what the docs said
  for that release.
- **Every release line since 2.0 is on the site.** 2.0, 2.1 and 2.2 came
  before versioning, so their copies were taken from each release's tag
  (`npm run freeze-docs -- X.Y.Z --from-tag`), with their links rewritten
  to this site's layout and links to repository files pointed at GitHub at
  that tag. Their words are as they were at the tag. Nobody is sent to a
  GitHub folder to read old docs.
- **A version switcher** on every docs page lists the latest line and each
  older one. It opens over the sidebar rather than pushing it down.
- **Frozen copies are not indexed.** Each frozen page has a canonical link
  to the same page under `/docs/` and `noindex`, is left out of the sitemap
  and out of site search, so a search never lands a reader on old
  instructions by accident.

## Consequences

Better:

- A reader on an older release can read the docs for what they run, and
  every page says which release it describes.
- The copies are plain files in the repository: they build without git
  history, review like any other change and need no step at deploy time.
- Forgetting the freeze fails a pull request instead of silently leaving a
  release undocumented.

Worse:

- **The repository grows by one copy of the docs per minor release**, about
  a dozen Markdown files each. Fine for years at this pace; if it ever is
  not, the oldest lines could be dropped from the site and linked at their
  tags instead.
- **Components are shared, not frozen.** A frozen page renders with today's
  run block, verdict cards and downloads table. If one of those changes in a
  way that contradicts an old release, the frozen page shows the new one.
  The words are frozen; the furniture is not.
- **`/docs/` follows `main`, not the tag.** A docs change merged between
  releases appears under "latest" before the release it describes. That
  was already true and is the price of fixing a mistake in the docs without
  cutting a release.
- Search finds only the latest docs. Someone looking for an old flag has to
  open that version from the switcher.

## Related

- ADR-0055, the move to plain Astro that makes a second copy of the docs a
  page route rather than a fork of a theme.
