# 0035. The guided flow is four calm screens, answered in words

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-03 |

## Context

ADR-0032 built the guided flow as six steps that printed the full report
inline and asked the operator to "type 1 or 2". The maintainer ran it and said
what was wrong with it, and he was right: too much information, tangled
together, cramped, and prompts that are easy to miss. The person using this
has just learned they may have malware on every machine they own. They are
not reading carefully, and a flow that needs careful reading fails them at the
moment it exists for.

A mock of a replacement was drawn first, one screen per step, and approved
before any of it was built.

## Decision

The prompt rules of ADR-0032 stand: only `yes` changes anything, only `q`
leaves, and input that ends stops the session. The shape around them changes.

**One question per screen**, with empty lines around it and the answer on a
line of its own after a `>`.

**Answers are words.** `computer` or `folder`, `yes`, `no`, `details`, in any
case. Never a number matched against a list. Enter only ever accepts a choice
that reads, such as the folders it found; it never accepts one that writes.

**Every step has a header**: which step of four, what it is, and, when it
changes nothing, a line saying so.

**A summary first.** Step 3 shows two counts, CONFIRMED and TO REVIEW, then
what was found as plain things grouped by where they are: "1 config file with
the payload hidden in it", "1 login item that starts the payload". No path, no
indicator, no section list. One sentence says what it means. The full list,
what and where on two lines each, appears only for `details`.

**Four steps, not six**: what to check, where, what was found, what to do
now. Credentials, rebuilding and the remote are the last screen, as a short
numbered list written from what is still there.

**A report is always saved**, to the home directory unless `--report` says
otherwise, and the last screen names it. The screens deliberately leave things
out, so the whole of it has to be somewhere.

**Plain words come from one place.** `Kind` is now fine-grained, one variant
per thing a person would name, and carries its own wording. A `[HIT]` cannot
be created without one.

**Lines are spans, never marked-up strings.** A line mixes words the tool
wrote with paths it found, and a path is chosen by whoever planted the file.
Text in a span is never parsed, so no file name can change how a line is
coloured or what it appears to say.

`polinrider check` is unchanged. It keeps the full, sectioned output for
people who want it and for the report.

## Consequences

Better: the screen a frightened person sees first is four lines of counts and
plain words, and the only question that can change anything is alone on the
screen with three words to choose from.

Worse:

- **The summary hides things on purpose.** Somebody who never types `details`
  never sees a path on screen. The report has them, and people do not open
  reports. For a `[review]` item, which only a person can judge, that matters:
  the review-only screen lists them in full for that reason.
- **Plain words are less exact.** "A login item that starts the payload" is a
  systemd unit, a launch item, an autostart entry or a system cron file. The
  person fixing it by hand needs to know which, and has to ask for `details`.
- **A report file now appears in the home directory on every guided run**,
  without being asked for. It names the paths on the machine. It is the right
  default for this flow and it is still a file nobody requested.
- **More screens means more scrolling.** The generous spacing that makes each
  question hard to miss also pushes the previous screen away. Nothing is
  cleared, so it is all in the scrollback, but it is not all in view.
- **Words have to be typed correctly.** `computer` is nine letters where `1`
  was one. First letters are accepted, quietly, and a typo asks again without
  doing anything. It is slower, and that was judged the right trade for a
  prompt nobody can answer by accident.
- **The wording is English and fixed.** Every phrase in `Kind::plain` is a
  sentence somebody will read while stressed, and none of them has been read
  by anyone but the maintainer and the author.

## Related

- ADR-0032, the guided flow changes something only on a typed yes: its prompt
  rules stand; its six steps and its numbered menu are replaced by this.
- ADR-0033, the banner and colour: spans are coloured by the same module, by
  tone and not by parsing.
- ADR-0034, what to do next: the last screen follows the same rule, that
  "rebuild" is said only when something proves the payload ran.
