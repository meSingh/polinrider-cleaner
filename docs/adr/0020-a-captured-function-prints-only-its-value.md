# 0020. A captured function prints only its value

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

The same bug appeared three times.

`ui_menu` printed the menu and echoed the choice, both on stdout. Called as
`choice="$(ui_menu ...)"`, the menu was captured into the variable and the
operator saw nothing while the terminal waited at an invisible prompt.

`pick_repo` was written the same way and shipped with the bug after `ui_menu`
was fixed. `ui_prompt` had a related version of it.

From the outside every one of these looks identical to a hang, which is the
worst way for it to fail: the reasonable response is to kill the process.

## Decision

Any function whose output is captured writes its **display to stderr** and only
its **return value to stdout**. That applies to `ui_menu`, `ui_prompt`,
`pick_repo`, `ask_owner`, `ask_mode` and `ask_roots`.

`ci/selftest-ui.sh` asserts it for each: run under `$( )`, the captured output
must equal the value alone.

## Consequences

Prompts do not appear in a redirected log, which is correct: a log of a
non-interactive run should not contain questions nobody was asked.

Anyone adding a function of this shape has to remember the rule. The self-test
is the reminder, and it is cheap to extend.

Splitting a function's two audiences, the terminal and the caller, is not
obvious in shell, where both default to the same place. It is worth stating
because the failure looks like a hang rather than like a mistake.
