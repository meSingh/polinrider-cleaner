# 0012. The operator chooses how thorough cleaning is

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

Two remedies exist and neither is right for everyone. An earlier version defaulted to the additive one and hid the choice behind a flag, then behind the fourth item of a menu.

## Decision

The question is asked at the point of cleaning, with both costs stated:

- **Remove it.** One commit per branch deleting the files. Reversible. The payload stays in the history.
- **Erase it.** The files leave every commit; every ref is force-pushed. Nothing left to check out, but every commit id changes.

Erasing touches **every** branch and tag, not only the flagged ones, because a payload left on any ref can be checked out.

## Consequences

Neither is the default, so neither is chosen by accident.

Removing leaves the payload reachable: an old commit can still be checked out and its `.vscode/tasks.json` will run on folder open. That is the whole risk, stated plainly at the point of choosing.

Erasing diverges every existing clone and breaks links to commits.

## Related

- [ADR-0013](./0013-history-rewriting-uses-filter-repo-when-it-is-installed.md), the mechanism
- [ADR-0002](./0002-exit-code-3-means-the-scan-could-not-run.md), why a failed clean is not a finding
