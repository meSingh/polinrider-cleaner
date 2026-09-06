# 0017. Long operations show git's own progress

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

Cleaning a repository starts with a bare clone. That used `git clone --quiet`,
which emits **zero bytes**.

On an organization scan every mirror is a fresh clone, so each repository
produced minutes of complete silence. There is no way to tell that from a hang,
and the reasonable response to a hang is to kill it.

Output was also being routed through a line-based filter that indents and dims
subprocess text. Git writes progress with carriage returns rather than newlines,
so that filter swallowed it entirely even when it was there.

## Decision

The clone uses `--progress`, and the repository size is printed first:

```
bare-cloning meSingh/Ghost, about 412 MB, nothing is checked out
```

Output from the cleaning tools reaches the terminal directly rather than through
the indenting filter.

## Consequences

Git's progress counter works as designed, on one updating line.

Those operations lose the indentation and dimming that the rest of the output
has, so they sit visually flatter than a finding. During a long clone, live
progress is worth more than consistent margins.

The size comes from the GitHub API and is skipped silently when unavailable, so
an unauthenticated run still clones, just without the estimate.
