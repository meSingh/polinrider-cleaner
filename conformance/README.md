# The conformance corpus

This is the specification. Not the shell scripts, not the Rust that replaces
them: this.

Each case declares a filesystem to build and exactly what a scan of it must
return — exit code, findings that must appear, findings that must **not**, and
which paths the run is allowed to touch. Every implementation runs the same
cases, so the port is finished when it agrees with the shell on all of them,
rather than when it compiles.

```bash
./conformance/run.sh                    # the shell implementation
./conformance/run.sh --impl rust        # the Rust binary
./conformance/run.sh --case font-masquerade   # one case, with the run output
./conformance/run.sh --diff             # every case, with output
```

Exit `0` all passed · `1` a case failed · `3` the harness could not run.

## No payload is committed here

Cases write `{{STRONG}}`, `{{WEAK}}`, `{{BADPKG}}` and `{{PAD}}`. The runner
substitutes real values out of [`ioc/`](../ioc/) when it builds the tree.

Two reasons. Working malware strings stay out of the repository, so cloning
this does not trip anyone's scanner. And the corpus cannot drift away from the
indicator set it is testing: change `ioc/strong.txt` and the fixtures change
with it.

## Adding a case

Cases are generated from [`build-cases.py`](./build-cases.py) so the set stays
consistent and a reviewer reads intent rather than JSON punctuation. Add a
`case(...)` block and re-run it.

Every case carries a `why`. It is not a description of the fixture, which the
fixture already gives you. It is the argument for why the expected result is
the **right** result. A case without that argument pins behaviour without
justifying it, which is how a corpus quietly canonises a bug.

Three kinds of case are worth writing:

**What it must catch.** The canonical infection, in each of the shapes the
campaign uses.

**What it must not catch.** Every false positive found in the field gets a case
so it stays fixed. A tool that cries wolf during an incident wastes the one
thing nobody has.

**What it deliberately misses.** `node-modules-is-not-scanned` and
`payload-in-an-ordinary-source-file-is-not-found` both assert a limitation.
They exist so the limitation is explicit and survives the rewrite in both
directions: the port should not silently start catching these, and should not
silently stop.

## What it cannot tell you

The corpus pins current behaviour, including any bug not yet found. It makes a
port faithful, not correct. That is the reason every case has to argue for its
expected value, and the reason a genuinely wrong answer here is worse than no
case at all.

## Host cases

Most cases describe a tree. A case with a `host` key describes a machine as
well: its process table, its sockets, its crontab, its system directories, as
files. These run without `--fs-only` and with `--host-state` pointing at those
files, which is how a check that reads "whatever is running" gets a fixed
answer to be measured against.

| File in `host` | Holds |
|---|---|
| `platform` | `linux` or `macos`. Decides which persistence locations are read |
| `processes` | one per line: pid, name, command line, tab-separated |
| `connections` | socket tool output, one connection per line |
| `crontab` | the user crontab |
| `git-config` | `key=value`, as `git config --global --list` prints it |
| `root/...` | stands in for `/`: `root/etc/cron.d/x` is `/etc/cron.d/x` |

A `home` map is written under the fixture's home directory, for startup files,
`~/.npmrc` and user-level persistence.

**An absent file is not an empty file.** Empty means the question was asked and
the answer was nothing. Absent means nobody asked, and the scan has to say so
with a `[review]` line. `<name>.absent` records that the tool which would
answer was not installed. `host_state()` in `build-cases.py` starts every case
from a quiet machine with every question answered, so a case changes only the
one thing it is about.

Host cases add three placeholders: `{{IMPLANT}}`, `{{IMPLANT_CUT}}` (the same
name cut to the 15 bytes the Linux kernel keeps) and `{{NETIP}}`.

**The shell implementation skips these**, and prints `skip` for each one so a
shell run cannot look as though it covered them. Only the Rust engine can be
handed a machine that does not exist. That has a cost, written into
[ADR-0029](../docs/adr/0029-host-state-is-read-through-one-boundary-and-can-be-supplied.md):
for the host checks there is no second implementation to disagree with the
corpus, so a wrong expected value here has nothing to catch it but its `why`.

What the host cases do not cover is the thin layer that actually runs `ps`,
`ss`, `lsof`, `crontab` and `git` on a real machine. The cases start on the far
side of it.

See [ADR-0026](../docs/adr/0026-2-0-0-is-one-binary-built-against-a-conformance-corpus.md).
