# 0033. Every run opens with the banner, and colour changes no character

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-03 |

## Context

1.x prints its wordmark on every run and colours findings by what they mean.
The Rust engine printed neither. ADR-0032 listed "no colour, no menus" as a
cost and left it there.

The first time the maintainer ran 2.0 it showed. The output opened with
`PolinRider local check - linux - scan`, `roots:`, `mode:`, four lines of
labels to decode before knowing whether the thing was about to move his files.
A `[HIT]` looked like an `[ok]`. `--version` printed a path. And the sandbox
script had piped `--version` through `head -1`, so the first thing the beta
ever printed for him was a Rust panic: `println!` dies when the reader goes
away.

## Decision

**The banner opens every run**: the wordmark, the author and the build. On
`check`, `clean`, the guided flow and `--version`. Not on a refusal, which is
an error on standard error and should be nothing else, and not in the report
file.

**The opening is sentences.** What system was detected, what kind of run this
is, and whether anything will be changed, in that order and before the first
finding: `DRY RUN, read-only. No changes will be made at this stage.`

**Colour carries meaning and nothing else.** Red is a confirmed finding,
yellow needs a person, green is checked and clean, grey is inventory. `Ui::paint`
wraps a line in colour and never alters a character of it, and a test strips
the codes and compares. So a terminal, a pipe and the report file hold the
same text, and the conformance corpus, which reads a pipe, is unaffected.

**Colour is off without being asked** when output is not a terminal, when
`NO_COLOR` is set, and when `TERM` is `dumb`. `PRC_ASCII` replaces the wordmark
with the name in letters.

**`--version` shows counts.** The version, the build, the platform and how
many indicators of each kind are loaded. Not where the files are, except when
they cannot be used, which is the one time the location is what needs fixing.

**Nothing is printed with `println!`.** Output goes through one function that
ignores a reader who has gone away, and `clippy::print_stdout` is denied so it
stays that way. A run piped into `head` ends quietly with the exit code it
would have had.

**All of it lives in `src/ui.rs`**, which runs no command, reads no file and
decides nothing about what is infected. ADR-0014's rule for the shell's `ui/`,
kept.

## Consequences

Better: the first screen answers "what is this about to do" before anything
else, and a confirmed finding cannot be mistaken for a passing check.

Worse:

- **Standard output now starts with a wordmark, piped or not.** A script that
  read the first line of the output gets block characters. The exit code is
  the interface for scripts and always was, and 1.x did the same, but anything
  that parsed 2.0's old first line is broken by this.
- **The terminal width is not known.** There is no way to ask without a
  dependency or unsafe code, so the wordmark is printed whenever the locale is
  UTF-8. On a terminal narrower than 75 columns it wraps and looks broken.
  `PRC_ASCII=1` is the way out, and nobody will know to set it.
- **`paint` recognises lines by their text.** A new kind of line is plain
  until somebody teaches it, and a message that happens to begin like a known
  one takes its colour. The rule that unknown text never borrows red limits
  the damage; it does not remove the coupling.
- **A closed pipe is now silent, which can hide a truncated report.** The run
  cannot tell `| head` from a reader that crashed. The report file is the
  complete record, when one was asked for.
- **Colour has only been looked at through `script` in the sandbox.** The unit
  tests prove what the codes wrap. Whether it reads well on a real terminal,
  light or dark, is a judgement nobody has made yet but the maintainer.

## Related

- ADR-0014, the presentation layer holds no logic: the rule this module is
  written to.
- ADR-0032, the guided flow: its "no colour" cost is what this record removes.
- ADR-0026, the conformance corpus: unaffected by design, since colour changes
  no character and the corpus reads a pipe.
