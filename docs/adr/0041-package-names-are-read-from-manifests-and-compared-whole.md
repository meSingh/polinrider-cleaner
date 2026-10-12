# 0041. Package names are read from manifests and compared whole

| | |
|---|---|
| **Status** | Proposed |
| **Date** | 2026-10-13 |

## Context

`ioc/bad-packages.txt` was matched the way 1.x matched it, as fixed strings
against the whole text of a file. It was also folded into `strong.txt` at load,
so every content check (tasks, hooks, build configs, extensions, persistence,
GitHub) asked the same substring question.

That is wrong in both directions for a package name. A campaign name that is
the start of a real package reports every project using the real one:
`sass-format` is inside `sass-formatter`. The weekly review has turned eight
known-malicious names away for exactly this
([2026-09-14](https://mesingh.github.io/polinrider-cleaner/indicator-reviews/2026-09-14/),
[2026-10-05](https://mesingh.github.io/polinrider-cleaner/indicator-reviews/2026-10-05/)),
proved the false positive against fixtures, and carried the gap forward every
week since as "needs a whole-name check against a parsed manifest". A name in a
`description` or `keywords` field reads as a dependency too, though nothing in
a description is installed.

## Decision

**A manifest is read for the names it installs.** `src/manifest.rs` reads
`package.json` and `package-lock.json` with a small JSON parser, and `yarn.lock`
and `pnpm-lock.yaml` line by line. From `package.json`: the package's own
`name`, the keys of `dependencies`, `devDependencies`, `optionalDependencies`
and `peerDependencies`, the `bundledDependencies` list, and the real package
behind an `npm:` alias. From a lockfile: v1 `dependencies` recursively with
`requires`, v2 and v3 `packages` keys after the last `node_modules/`, yarn v1
and berry entry headers and berry `resolution:` lines, pnpm `packages:` and
`snapshots:` keys in the v5, v6 and v9 spellings and the dependency maps under
`importers:`. Each name is compared whole, case as written, against
`bad-packages.txt`. A hit lists the names it found under the finding.

The parser holds its nesting in a vector rather than on the call stack and
refuses anything deeper than 256 levels, so a hostile manifest cannot exhaust
the stack. JSON escapes are decoded as part of reading a string, so a name
written in `\u` escapes is read as the name (ADR-0040).

**A manifest that cannot be read is review, never clean.** Its text is matched
as 1.x matched it, so a campaign name anywhere in it is still a confirmed hit,
and a review line says the reading was by text: "could not be read as a
manifest, so package names were matched as text". A file nobody could parse is
one nobody can vouch for, and a reader that gave up must not turn a finding 1.x
made into a clean result.

**Outside a manifest, a package name counts only when it stands whole.** The
names are no longer folded into `strong`. `has_strong` matches `strong.txt` as
fixed strings, as before, and a campaign package name where it is bounded on
both sides by something that cannot be part of a package name, and is not the
second half of somebody's scope (`@other/name` is another package). `node
x/evil-pkg/index.js`, `npm i evil-pkg` and `"evil-pkg":` match; `evil-pkg-utils`
and `not-evil-pkg` do not. The whole-file content match is kept: a campaign
name in a task, a hook, a config or an extension is still a confirmed
indicator. What changed is only that a longer name is no longer the shorter
one.

**On GitHub, the same reading.** `git grep -F` with `strong.txt` alone confirms
as before. A second `git grep -F` with the package names lists candidates, and
each is read once in a single `git cat-file --batch`: a manifest for its names,
anything else for the name standing whole. The history pass behind `erase`
asks the same question of each file version, so a `package.json` that only
mentions a campaign name in its description is not cut out of every commit.

## Consequences

Better:

- The eight rejected names can now be carried. They are not added here: adding
  an indicator is the weekly review's job and needs its source, but the reason
  for turning them away is gone.
- A description, a keyword list or a README in a package no longer reads as a
  dependency.
- A finding names the package, so the reader does not have to search a
  thousand-line lockfile for it.

Worse:

- **A parser is code to maintain.** Four formats, three pnpm spellings and two
  yarn ones. A format a package manager adds later reads as unparseable, which
  is a review line on every project using it until the reader learns it. That
  is noisy rather than silent, which is the direction to be wrong in.
- **The line readers are not YAML parsers.** A pnpm lockfile written with
  unusual indentation, or a yarn header split across lines, can yield the
  wrong names. pnpm and yarn write these files themselves and do neither; a
  hand-edited lockfile is the case this misses.
- **A malformed manifest that names a longer package is still a false
  positive.** The fallback is 1.x's substring match, by design, with a review
  line beside it. If the eight names are added and a project has a lockfile
  with conflict markers that depends on `sass-formatter`, it reads as
  infected until the conflict is resolved.
- **Whole-name matching outside manifests can miss a name glued to
  something.** A tarball called `evil-pkg-1.0.0.tgz` in a hook no longer
  matches `evil-pkg`, because `-1` could continue a name. Inside a lockfile
  the same tarball's package is read by name, which is where it matters.
- **Other ecosystems are still text.** `bad-packages.txt` carries two PyPI
  names and a commented Packagist block. Only npm's files are parsed;
  `requirements.txt`, `pyproject.toml` and `composer.lock` are not read for
  packages at all, as before.
- **GitHub reads more.** Every file holding a package name anywhere is read
  out of git once more, in one batch per branch. The candidates are files
  that already contain a campaign name, so on a clean organization the list
  is short.

## Alternatives rejected

- **Keep substring matching and keep rejecting names.** That is what the
  review has done for a month, and it leaves eight known-malicious names
  unmatched with no end date.
- **Whole-name matching on the raw text of every file, no parser.** Fixes the
  substring direction and not the description one: a description is still
  text with the name standing whole in it.
- **Ask the package manager.** `npm ls` on a machine suspected of an npm worm
  executes the thing under suspicion, and needs node_modules installed.
- **Drop package names from every content check outside manifests.** Loses a
  campaign name in a hook or a task, which has no innocent reading, to fix a
  false positive whole-name matching already fixes.

## Related

- ADR-0040, escape codes, which a JSON manifest can also use.
- ADR-0036, the GitHub check this adds a pass to.
- ADR-0039, why every new conformance case argues for itself.
