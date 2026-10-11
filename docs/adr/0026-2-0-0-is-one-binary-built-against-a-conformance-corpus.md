# 0026. 2.0.0 is one binary, built against a conformance corpus

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-21 |

## Context

Three problems arrived together, and they have one answer.

**The tool is confusing to run.** There are seventeen separately runnable
scripts. `github-org-recovery/` and `github-account-recovery/` are near-duplicate
sets of six files with identical names; five of the seven pairs differ by nothing
or by a single owner-type argument. `polinrider.sh` exists to hide all of this,
but the folders are still there, still documented, and still the first thing a
reader sees. The maintainer, who wrote it, could not tell whether to run the
central entry point or the individual scripts while cleaning a backup drive.

**The documentation is scattered.** 2,933 lines across 25 Markdown files, plus
930 lines of architecture records, plus a 934-line README. Every track has its
own README that repeats parts of the main one.

**Shell is producing a specific class of bug.** Thirteen of the twenty-five
records in this directory exist because something broke. Sorting those failures
by cause:

Prevented by a type system: `grep -c` printing `0` while exiting `1`, so
`|| echo 0` produced `"0\n0"` and arithmetic on it failed. `printf` reading a
format string beginning with `-` as a flag. A path rewrite replacing every
slash when `TMPDIR` was unset. Root lists held as space-separated strings,
splitting `Application Support` in half. Three separate cases of a function
printing its display to stdout where `$( )` captured it, which is the only
reason ADR-0020 exists. A loop reading from a file, so a prompt inside it
consumed the next record instead of the operator's answer. BSD and GNU `date`
disagreeing. A script edited while running, which ADR-0023 works around.

Not prevented by a type system: `-not -path` where `-prune` belonged (ADR-0025).
One exit code meaning both "confirmed" and "could not run" (ADR-0002). A blank
line reading as a quit (ADR-0021). Restore points that were themselves infected.

Roughly eight against five. The worst failure of the whole project is in the
first group: a loop that consumed its own stdin reported repositories as
"left alone" that it had never scanned. A security tool that silently reports a
clean result for work it did not do is the failure that ends a tool's
credibility, and it was a shell idiom rather than a mistake in reasoning.

Windows is a separate hole. `check-windows.ps1` covers Run and RunOnce keys, the
Startup folder and scheduled tasks. It does not check WMI event subscriptions,
Winlogon and Userinit, Defender exclusions, IFEO, AppInit_DLLs, BootExecute or
alternate data streams. WMI subscriptions live in the WMI repository rather than
in a file or a registry key, so they survive both a reboot and any cleanup that
only looks in the usual places. Defender exclusions matter because this campaign
family adds exclusions covering its own directories, so a machine reports itself
protected while the product has been told to ignore the payload.

## Decision

2.0.0 is a single binary written in Rust, with the guided flow inside it, built
on a branch and merged only when it is provably not a regression.

**One entry point.** `polinrider` with subcommands. Running it with no arguments
enters the guided flow: triage, machine check, credential rotation, remote
cleanup, verification, prevention, each step moving to the next inside the same
session. No second command, no second terminal. The per-track scripts and the
duplicate recovery folders are deleted, not wrapped.

**A conformance corpus is the specification.** Fixture trees committed as data,
each with its expected verdict, exit code and the exact set of paths the run may
touch. The corpus is written first, against the shell implementation, and the
Rust implementation is finished when it produces identical results on every
fixture. Divergence between the two is a bug in one of them, found by a diff
rather than by a user.

**Dry-run safety is a type, not a convention.** A scanner parameterised on its
mode exposes no method capable of writing when it is in dry-run mode, so
"quarantined a file during a dry run" fails to compile rather than requiring a
reviewer to notice it. This is the property that chose Rust over Go.

**Documentation is one site.** mdBook, published to GitHub Pages, sources in
`docs-site/src/`. Chosen over Material for MkDocs, which entered maintenance
mode in November 2025, and over anything on a Node toolchain, which is
indefensible for the cleanup tool of a Node supply-chain worm even as a build
dependency. mdBook is a single Rust binary, so the documentation build and the
tool share a toolchain.

**Sequencing.** The corpus first, against the shell. Then consolidation. Then
Rust one track at a time, Windows first because there is no working
implementation to regress and the gap is largest. 2.0.0 merges when every track
is Rust and the corpus is green on macOS, Linux and Windows. 1.x stays on `main`
and keeps receiving indicator updates throughout.

## Consequences

Better: one thing to run, one place to read, and a class of defect that has
caused most of this project's incidents becomes unrepresentable. Windows stops
being a second-class script that the entry point cannot even launch.

Worse, and worth stating plainly:

- **"Shell only, nothing to install" stops being true.** It is on the README,
  the social card and every post about this tool, and it is not marketing: it is
  the argument for running unknown code on a machine you believe is compromised.
  A binary replaces "read this before you run it" with "trust the build
  provenance". The releases are already attested, and the source stays readable,
  but this is a real loss and the documentation must say so rather than quietly
  drop the claim.
- **A rewrite produces no new users.** The tool has two stars. The work that
  gains users is a pull request to the campaign's primary dossier, which lists
  no tool at all. That work must not stop while this happens.
- **The corpus can encode a wrong answer.** It pins current behaviour, including
  any bug not yet found. It makes the Rust port faithful, not correct. Every
  fixture needs a reason recorded for why its expected verdict is right.
- **Two implementations exist during the port.** Both need indicator updates.
  The weekly review touches `ioc/`, which both read, so this is survivable, but
  a fix to scanning logic has to be made twice or deliberately made once.
- **Type-state prevents writes, not wrong verdicts.** It cannot catch a prune
  that skips a directory it should have entered, or an exit code that means two
  things. Those still need the corpus and review.

## Related

- ADR-0020, a captured function prints only its value: exists only because shell
  conflates a return value with output. The bug class this decision removes.
- ADR-0024, a prompt inside a loop must not share its stdin: the silent-skip
  failure, and the strongest single argument here.
- ADR-0025, walk the filesystem once: a logic bug a compiler would not have
  caught, and the reason the corpus matters more than the language.
- ADR-0023, the entry point parses before it runs: becomes unnecessary, since a
  compiled binary cannot be edited mid-run.
