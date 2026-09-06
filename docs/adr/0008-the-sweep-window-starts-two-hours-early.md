# 0008. The sweep window starts two hours early

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

Restoring branches needs a starting time. Too late and the first wave is missed; too early and the sweep returns months of unrelated activity.

## Decision

T0 is two hours before the earliest push recorded on a flagged branch, computed and filled into the generated commands.

## Consequences

**Two hours is an assumption, not a measurement.** It is a margin for staging that happened shortly before the visible push.

If the first hostile push is older than the events API remembers, the window starts too late and the restore target is itself infected. `restore.sh` refuses those rows and marks them `MALICIOUS_TARGET`; seeing any means widening `--since` and rebuilding the plan.

The generated plan states this where T0 appears, rather than leaving it to be discovered.

## Related

- [ADR-0010](./0010-restore-targets-are-read-before-they-are-offered.md), which catches the same failure by reading the target
