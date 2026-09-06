# 0018. Engines hide their standalone guidance when driven

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

Each engine ends by telling you what to do next: `gh-scan.sh` prints a numbered
"Next" block, `triage-filter.sh` tells you to read the matched content,
`next-steps.sh` prints the cleaning commands.

That is right when someone runs `lib/gh-scan.sh` directly. Driven from
`polinrider.sh` it is wrong twice over: the entry point has already run
`triage-filter.sh` itself, and it is about to offer those same actions as a
menu. The operator reads three sets of instructions, then gets asked the
question anyway.

## Decision

`polinrider.sh` exports `PRC_EMBEDDED=1`. Each engine wraps its closing guidance
in a check for it. The guidance is **hidden, not deleted**: running any engine
directly still prints it.

## Consequences

Two output shapes per engine, so a change to the guidance has to be checked
both ways.

The alternative was deleting the blocks, which would have made the individual
scripts worse for the people most likely to read them: anyone auditing the tool
runs the pieces separately.

`PRC_EMBEDDED` says nothing about behaviour, only about who is listening. No
engine may change what it *does* based on it.
