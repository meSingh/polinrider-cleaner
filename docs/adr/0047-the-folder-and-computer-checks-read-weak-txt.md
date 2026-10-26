# 0047. The folder and computer checks read `weak.txt`

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-26 |

## Context

`weak.txt` holds strings the campaign uses that legitimate projects use too:
blockchain RPC hosts, `windowsHide`, `stdio: 'ignore'`, `folderOpen`.
`ioc/README.md` has always said a match is a `review` line. The CI scanner and
the GitHub check do that. The folder and computer checks never read the list:
1.x defined `has_weak` and never called it, and 2.0 carried the gap over. The
2026-10-12 indicator review found it, and every review since has had to say
that review-tier indicators do not reach a local scan.

## Decision

A last pass over the files the walk lists, with the CI scanner's rule: every
file of up to 1 MB that is not binary (no NUL byte in its first 8 KB), the
first weak string per file, matched as written and not decoded from escapes.
A match is one review line naming the string and the path. It never confirms
anything and never moves anything.

Set aside, so the pass does not report the operator's own material:

- what the GitHub check already treats as detection tooling, read relative to
  the root the file was found under: `ioc/*.txt`, `.github/polinrider/`, the
  scanner's own scripts, documentation (`*.md`, `docs/`, READMEs);
- anything under `.vscode/`: `tasks.json` has its own stricter check, and
  `folderOpen` in `settings.json` is a false positive the corpus already pins;
- a file another check has already reported.

It is a progress stage of its own, so the counts on the progress screen move
from 12 to 13 (computer) and 9 to 10 (folder).

## Consequences

- `weak.txt` now does what the README says, in all three engines.
- **More exit-1 results on ordinary projects.** `windowsHide` and
  `stdio: 'ignore'` appear in real build scripts. That is what the list is for,
  and the CI scanner has always said it.
- **A home-folder check reads more file contents than before**: every small
  text file it lists, outside the pruned directories. Bounded at 1 MB a file;
  the cost is time on a large home folder.
- The local pass differs from the CI scanner in two places: it skips
  `.vscode/` and documentation. The CI scanner reads both.
- Weak strings are not decoded, so an escape-coded weak string is not seen.
  The same as the CI scanner, for the same reason: decoding would add a line to
  every bundle that escapes a common word.

## Related

- ADR-0040: escape-coded loaders, matched decoded for the strong tier only.
- ADR-0045: the small files the checks read are compared by hash.
