# 0011. Pre-attack commits are fetched, not assumed present

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

`restore.sh` looks for the pre-attack commit in the local mirror. `git clone --mirror` fetches only what is reachable from a ref, and after a force-push that commit is reachable from nothing.

Checked against a real incident: 10 pre-attack SHAs in the ledger, 0 present in their mirrors. The restore path did not work for the case it exists to handle.

## Decision

`preserve-restore-points.sh` fetches each pre-attack commit by SHA and anchors it under `refs/polinrider/pre-attack/`. It fetches and never pushes.

## Consequences

GitHub serves unreachable objects by SHA until it garbage-collects them, so this only works while they are still there. Anything reported `GONE` cannot be restored and the tool says so.

The anchor matters as much as the fetch. Without a ref the object is unreachable locally too, and the next `gc` discards exactly what was just retrieved.

This must happen before the evidence directory is cleared, which is the tension with [ADR-0003](./0003-evidence-lives-in-a-temporary-directory.md).

## Related

- [ADR-0003](./0003-evidence-lives-in-a-temporary-directory.md), which makes the evidence directory temporary
