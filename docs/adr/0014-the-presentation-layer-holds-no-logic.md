# 0014. The presentation layer holds no logic

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

Output was unreadable: timestamps, git chatter and confirmed findings all printed flush left in one colour, with no way to see where one operation ended and the next began.

Fixing that means a lot of formatting code, which is a lot of code between an auditor and the part that matters.

## Decision

All drawing lives in `ui/`. `theme.sh` holds the palette, symbols and terminal capability detection; `render.sh` holds the drawing functions. Neither reads a repository, runs git, or decides whether something is infected.

Indentation carries meaning: sections flush left, steps at two, results at six and marked, subprocess output at six and dimmed. Everything a subprocess prints goes through a filter, so git output cannot sit at the same visual level as a finding.

No dependency was added. `gum` is better than this and is a binary someone must install; asking for that in a tool whose subject is supply-chain compromise is the wrong trade. `dialog` and `whiptail` need ncurses and are absent on macOS. `tput` is used for terminal width and nothing else.

## Consequences

An audit can skip `ui/` entirely. `ci/selftest-ui.sh` fails the build if an external command appears there.

Facts the header displays, such as whether `gh` is signed in, are gathered by the caller and passed in, because gathering them would mean running a command.

Colour carries meaning, so it is bound once: red is only ever a confirmed finding. Unrecognised text is dimmed, so it can never borrow the colour of a hit.

Everything degrades: no colour off a terminal, under `NO_COLOR`, or under `TERM=dumb`; ASCII marks without a UTF-8 locale; plain text below 75 columns. Piped output contains no escape sequences at all.
