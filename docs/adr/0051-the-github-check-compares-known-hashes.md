# 0051. The GitHub check compares the small files against the known hashes

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-26 |

## Context

ADR-0045 made the folder and computer checks compare every `tasks.json`, font
file and build config with `hashes.txt`, and recorded the cost that the GitHub
check did not. A repository carrying a published campaign file with no
indicator string was confirmed on a local checkout and missed on GitHub, where
the cleanup happens. The weekly reviews' "blob-ID tier" asked for the same
thing by another route: git blob IDs need the original bytes to compute,
while the sources publish SHA-256.

## Decision

The GitHub check reads each `.vscode/tasks.json`, font file and build config
on every branch and tag, hashes it, and confirms one that matches, with the
file's label. It counts as a confirmed path like a fake font, so `restore`
will not move a branch to a commit carrying it, `remove` deletes it, and the
history pass `erase` relies on finds it in older commits too.

## Consequences

- A known small file is found where the cleanup is, and taken out by every
  fix.
- More blobs read on each branch: the configs, fonts and tasks files. They
  are small and few; the fonts were read already.
- Still only those three kinds of file, as on the local side. A known file of
  any other kind is not hashed anywhere. The blob-ID tier is closed by this
  rather than built: it would match the same files by a hash nobody publishes.

## Related

- ADR-0045: the same comparison on this computer and in folders.
- ADR-0037: how a fix is planned and believed.
