# 0007. Forks and archived repositories are in scope

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

Both could reasonably be excluded. A fork is mostly someone else's code; an archived repository is read-only.

## Decision

Both are scanned by default. `--no-forks` excludes forks.

## Consequences

A fork is a repository you own and can push to, so it can carry the payload and spread it. Scanning it is correct.

**An archived repository cannot be pushed to, so cleaning one will fail.** Scanning it is still right, because knowing it is infected matters even when the fix is to unarchive first. The failure is currently reported as a rejected push rather than being anticipated, which is not ideal.
