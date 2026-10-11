# 0030. 2.0 does not checkpoint a scan

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-03 |

## Context

1.x has `--state` and `--resume`. ADR-0025 added them after a scan of a backup
drive ran for six hours and died at logout with nothing saved.

That record names two causes. The scan was slow because eight checks each
walked every root with `-not -path`, which filters what `find` prints without
stopping it descending into `node_modules`. And nothing was checkpointed, so
the six hours were lost. ADR-0025 fixed both: one pruned walk, and a state
directory a later run could resume from.

The Rust engine kept the first fix and never had the second. It walks once,
never enters a pruned directory and holds the file list in memory. `--state`
and `--resume` were refused with "not implemented yet", which promised they
were coming.

Checkpointing was the cure for a scan that took hours. The scan no longer takes
hours, and the checkpoint has costs of its own that ADR-0025 lists: a state
directory holding an inventory of every path on the machine, and a resumed run
that does not see files added since the original walk. The second is worse
than it sounds for this tool. The most important bug found during the port was
in exactly this machinery: two runs in the same second shared a state directory,
and the second inherited the first one's checkpoints, skipped every check and
reported clean for work it never did.

## Decision

2.0 does not checkpoint a scan. Every run starts from the beginning and walks
the filesystem once.

`--state` and `--resume` stay recognised and are refused, exit 3, with a
message saying they were removed in 2.0. Not "unknown option": somebody arriving
from 1.x should be told what happened to the flag. And never ignored, because a
flag that silently does nothing is how a scan that started over gets believed
to have resumed.

Mandeep made this call on 2026-10-03.

## Consequences

Better: a class of bug is gone with the code that could have it. No run can
inherit another run's progress, and nothing writes an inventory of the machine
to disk.

Worse:

- **An interrupted scan starts over.** For a project directory that is seconds.
  For a slow external drive with many large files it may not be, because the
  hash sweep reads every file between 10 MB and 300 MB, and the Rust engine
  does not cap that at 200 files the way the shell does. If a real scan turns
  out to take long enough to be interrupted, this decision is the first thing
  to revisit, and the honest fix may be to bound the sweep and not to bring
  checkpoints back.
- **`--background` loses its partner.** In 1.x a detached scan that died at
  logout was resumed. It is still listed as not implemented; without resume it
  is worth less, and it may follow these two out.
- **A 1.x habit breaks.** Scripts that pass `--state` fail with exit 3 until
  the flag is removed from them. That is deliberate, and it is still a break.
- **This has not been measured on a large drive.** The claim that the scan is
  now fast rests on the pruned walk and on the sandbox, not on the backup drive
  that started all this. The three-machine test before release is where it gets
  measured.

## Related

- ADR-0025, walk the filesystem once, with -prune, and checkpoint it: 2.0
  keeps the walk and drops the checkpoint. That record still describes 1.x on
  `main`, so it is not superseded.
- ADR-0026, 2.0.0 is one binary built against a conformance corpus: three
  refusal cases pin the behaviour of the removed flags.
