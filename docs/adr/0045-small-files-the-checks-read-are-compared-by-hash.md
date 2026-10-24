# 0045. Compare the small files the checks read against the known hashes

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-25 |

## Context

`hashes.txt` was compared in two places only: the install paths in
`implant-paths.txt`, and a sweep of files between 10 MB and 300 MB, sized for
the second-stage binaries. Every hash published for a smaller file could not
fire. Weekly reviews turned such hashes away from 2026-09-21 on: a poisoned
`.vscode/tasks.json` (799 bytes) and a fake font (9 KB), both published with
their SHA-256 by OpenSourceMalware. The `print.cjs` dropper already in the
file never fired either. The file's header promised more than the code did.

Hashing every small file is not an option. A home folder scan lists hundreds
of thousands of files and reads very few of them; hashing all of them would
read everything it lists.

## Decision

The hashes load with the other indicators. The three checks that already
read a file whole compare its hash too, at any size:

- `.vscode/tasks.json`: a match is a confirmed editor task, moved on `--apply`
  like any other.
- font files (`.woff`, `.woff2`): a match is a confirmed fake font, even if
  the file starts with a font's magic bytes.
- build configs: a match is a confirmed config that `clean` will not strip,
  because there is no indicator string to cut at. The advice is to re-clone.

The finding names the label from `hashes.txt`. The 10 to 300 MB sweep is
unchanged. The two OpenSourceMalware hashes are added, and the header now
says where a match is looked for.

The corpus cannot hold the published bytes, so a case can list fixture files
under `known_hashes`. The runner copies `ioc/`, adds those files' hashes to
the copy and runs with `--ioc` pointing at it. `ioc/` itself is never written.

## Consequences

- A small published file is confirmed by what it is, not only by the strings
  in it. Today this adds less than it sounds: the published `tasks.json` also
  carries a `strong.txt` string, and the published fake font fails the
  magic-byte check, so both were already confirmed. What changes is that the
  next published small-file hash of these kinds is usable the day it is
  published, and the finding says which known file it is.
- **A small file of any other kind is still not hashed.** A loader script
  under 10 MB, the `print.cjs` dropper included, is found only by its strings
  or its name. A conformance case pins this limit. Closing it needs a size
  list or a bounded sweep of script files, which is its own decision.
- **The GitHub check does not use hashes at all**, and neither does the CI
  scanner (`ci/scan-workspace.sh`). A repository carrying a known small file
  with no indicator string is confirmed on a local checkout and missed on
  GitHub. Reading blobs and hashing them in the remote check is possible and
  was left out of this change.
- Each tasks.json, font and build config is hashed as well as read. These
  files are small and few, so the cost is negligible next to the walk.
- A tasks.json that is not valid UTF-8 is now read lossily instead of being
  skipped without a word, the same as build configs already were.

## Related

- ADR-0040: escape-coded loaders, the other change that widened what the
  content checks see.
- The 2026-09-21 indicator review, which first recorded the gap.
