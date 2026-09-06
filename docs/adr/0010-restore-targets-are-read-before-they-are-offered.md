# 0010. Restore targets are read before they are offered

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

The obvious restore target is the commit immediately before the hostile push. The campaign arrives in waves, so that commit is often the previous wave.

## Decision

Every recovered commit is scanned for the obfuscator markers and for a `.woff2` that does not begin with the `wOF2` magic, then recorded as `CLEAN` or `INFECTED` in `restore-targets.tsv`. Only the earliest clean commit per repository is offered.

## Consequences

On the account this was built against, 2 of 10 candidates were already infected, and both repositories had an earlier clean commit. Restoring to the immediate predecessor would have put the payload back on two repositories.

`gh-restore.sh` already refused a target it saw pushed inside the sweep window. That lineage check holds, but cannot see a payload committed before the window opens. Reading the tree is strictly stronger, so both run.

Where no clean target exists, the tool says so and routes to removal instead.
