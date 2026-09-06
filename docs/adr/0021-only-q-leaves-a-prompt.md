# 0021. Only an explicit q leaves a prompt

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

Every menu treated an empty line as "stop", the same as `q`.

Reading a long findings file means pressing Enter repeatedly to page through it.
One keypress past the end lands on the menu, and the run ended. The work is not
lost, but the whole scan has to be repeated to get back.

A typo, or a number larger than the menu, also ended the run silently.

## Decision

Only `q` and end of input leave a prompt. A blank line re-asks. A typo or an
out-of-range number re-asks and says what is wrong.

## Consequences

There is no way to leave by accident, and no way to leave without meaning to.

End of input still exits, so a piped or scripted run cannot loop forever on
input that will never be valid. The self-test checks that specifically.

A blank line re-asking is not always right: at "GitHub organization" during a
scan-everything run, blank means "skip". That case passes an explicit flag
rather than changing the default, because "ask me again" is the safer reading of
a stray keypress.
