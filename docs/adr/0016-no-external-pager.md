# 0016. No external pager

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

Reading the findings opened `$PAGER`, falling back to `less`.

Someone who has never used a terminal pager cannot guess how to leave one, and
on a machine where `$PAGER` is `vi` they are genuinely stuck: the reflex is to
kill the terminal, which abandons the scan. This tool is run by people in the
middle of an incident, often on a machine that is not their usual one.

## Decision

`ui_pager` reads the file with the shell alone and prints, on every page, which
key does what:

```
  24 of 312 lines. Enter for more, q then Enter to stop:
```

`$PAGER` is never consulted. Off a terminal it prints the file whole, because
paging into a pipe or a log helps nobody.

## Consequences

No search, no scrollback, no line numbers. For a long triage file that is worse
than `less` for anyone who knows `less`.

In exchange nobody can get trapped, and behaviour does not depend on an
environment variable set years ago for something else.

The full path is printed above the output, so anyone who wants a real pager can
open it themselves.
