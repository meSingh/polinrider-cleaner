# 0022. q quits, b goes back

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

`q` meant "go back" in the repository picker and "stop" in the action menu.

In the action menu, stopping returned through the call stack, which is exactly
what the last option, "Stop here", does. So `q` and the last option were
indistinguishable, and there was no way to leave the tool from a submenu without
walking back up through it.

## Decision

Three answers, three exit codes from `ui_menu`:

| | |
|---|---|
| `0` | a choice, on stdout |
| `1` | back one level, from `b`, offered only where a level above exists |
| `2` | quit the tool, from `q` |
| `3` | no input at all, from end of input |

`q` ends the run wherever it is typed, through `quit_now`, which exits with the
verdict code the normal path would have used. `b` is offered only where going
back means something, and is refused with an explanation elsewhere.

## Consequences

`quit_now` has to be called from the main shell. `ui_menu`, `pick_repo` and
`ask_mode` are all called inside `$( )`, and an `exit` there ends only the
subshell, leaving the caller to continue with an empty variable. They return a
code and the caller does the exiting.

Codes 2 and 3 are separate because "the operator chose to stop" and "nobody was
there to answer" deserve different exit codes. Running with no input at all is a
usage error and still exits 2; typing `q` exits on the verdict. Merging them
broke that, and the self-test caught it.
