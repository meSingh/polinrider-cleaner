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

It also only covers the filesystem checks. Runs use `--fs-only`, because the
live checks — processes, sockets, npm config, crontab, `$HOME` persistence —
describe the machine the suite is running on, and a result that depends on
what happens to be running is not a specification. Those need their own
approach, most likely injectable system state in the Rust implementation.

See [ADR-0026](../docs/adr/0026-2-0-0-is-one-binary-built-against-a-conformance-corpus.md).
