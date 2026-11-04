# 0055. Build the documentation site with plain Astro, not Starlight

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-11-04 |
| **Supersedes** | [ADR-0028](./0028-the-documentation-site-is-astro-starlight.md), for the Starlight part. Astro and the Node toolchain stay |

## Context

ADR-0028 chose Starlight because a documentation site needed search, a
sidebar, themes and code blocks, and Starlight had all four. By November the
site had grown past what a documentation theme is for:

- **Four sections that are not documentation.** News (with indicator reviews
  and articles published elsewhere), Releases and the campaign pages each
  wanted their own layout. Under Starlight every page sat in one mixed
  sidebar, and `src/routeData.ts` rewrote that sidebar per section.
- **A home page for a frightened reader.** The design that was approved puts
  three things on the first screen: what it checks, the commands and what
  happens after the scan. Starlight's splash template, `Hero` and
  `PageTitle` overrides and a `routeMiddleware` were all fighting its own
  markup to get there.
- **Six component overrides and a CSS file of `!important`-free but
  selector-heavy fixes** for Starlight's layers. Each Starlight upgrade could
  break one silently.
- **Versioned docs** (ADR-0056) need two copies of the same pages at
  different addresses with one sidebar each. Starlight has no versions.
- Starlight's page schema drops front matter it does not know (`date`,
  `review`, `version`), so the News and release lists read the files
  through `import.meta.glob` instead of the content collection.

## Decision

The site is plain Astro. Every page's HTML is in `docs-site/src/layouts/`
and `docs-site/src/pages/`, the design in `docs-site/src/styles/`.

- **Content collections**, one per section (`content.config.ts`): `docs`,
  `docsVersions`, `campaign`, `news`, `reviews`, `releases` and
  `elsewhere`. Each declares the front matter it uses, so nothing is read
  around the schema.
- **Markdown**: Astro's own processor, with one small plugin for the four
  `:::note`, `:::tip`, `:::caution` and `:::danger` callouts the pages use
  (`src/markdown/asides.mjs`). Code is coloured by Astro's built-in Shiki,
  both themes at once, switched in CSS.
- **Search**: Pagefind, run on the built site at the end of the build
  (`src/markdown/pagefind.mjs`), with our own search box on ⌘K. The index is
  static files; nothing is sent anywhere.
- **Old addresses keep working.** Astro writes a redirect page, with a
  canonical link, for every address that moved. GitHub Pages serves files
  only, so a page that refreshes is the redirect there can be.
- **A link check after every build** (`scripts/check-links.mjs`). Starlight
  did not check links either; the comment in `docs.yml` that said Astro
  fails on a broken link was wrong, and the first run of the check found
  three dead links on the live site.
- The things that must not change are carried over unchanged in behaviour:
  release dates from the signed tag (`src/releaseDate.ts`), the RSS feed,
  the social card, per-page titles and descriptions, the sitemap, the
  verification tags, the Plausible switch and `third-party-notices.txt`.

## Consequences

Better:

- Every element on every page is ours, so a design change is an edit to
  one file, not an override of a theme's internals.
- **No new code entered the dependency tree.** `@astrojs/mdx`,
  `@astrojs/sitemap`, `@astrojs/markdown-satteri` and `pagefind` were
  already installed as Starlight's own dependencies, at the same versions;
  removing Starlight took 73 packages out of the lockfile, including
  Expressive Code.
- Section front matter is validated, so a release page without a `version`
  fails the build instead of disappearing from the list.

Worse:

- **We own what Starlight gave for free**: the keyboard pattern of the tabs,
  the focus handling of the search dialog, the table of contents, the
  sidebar on a phone and the theme script that runs before the page paints.
  Each is a few dozen lines in `src/components/`, and each is ours to keep
  accessible.
- Starlight's accessibility and i18n work no longer arrives with upgrades.
  The site is English only and expects to stay that way; if it ever needs a
  second language, this decision should be revisited.
- Pagefind's search box is our own markup over Pagefind's JavaScript API,
  not Pagefind's default UI, so Pagefind's UI improvements do not arrive by
  themselves either.

## Related

- ADR-0028, which this supersedes for the theme. Its reasons for Astro and
  for a Node toolchain outside the container still hold.
- ADR-0056, the versioned documentation that this layout makes possible.
