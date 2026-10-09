---
title: Contributing
---

The most valuable contributions to this project are not code.

## An indicator we are missing

[Open a missed-detection issue](https://github.com/meSingh/polinrider-cleaner/issues/new?template=missed-detection.md)
with the source you saw it in. New indicators are reviewed against the
classification rule on [the indicator set](/campaign/indicators/): `strong.txt`
only when a match means infection with no plausible alternative.

## A false positive

[Open a false-positive issue](https://github.com/meSingh/polinrider-cleaner/issues/new?template=false-positive.md).
These matter more than they look. A tool that cries wolf during an incident
wastes the one resource nobody has, and the
[known ones](/reference/false-positives/) were all found this way.

## A fix to these docs

Every page has an edit link in the top right. It opens the Markdown source in
GitHub's editor and turns your change into a pull request. No local setup, no
toolchain.

## Code

**Run everything in the sandbox.** This tool quarantines files, rewrites git
history and walks `$HOME`, and its test suite exercises those paths. Testing it
directly on your workstation is one bad path expansion away from moving your
real files.

```bash
./polinrider-sandbox --all     # fmt, clippy, unit tests, the corpus, shell lint
./polinrider-sandbox --beta    # the binary installed in the container, with a sample
./polinrider-sandbox           # an interactive shell in the sandbox
```

The repository is mounted read-only, `$HOME` belongs to the container, and
networking is off unless you ask for it. It needs Docker, which is a
development dependency only: the released binary runs on a bare machine with
nothing installed. See
[ADR-0027](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0027-development-and-testing-happen-in-a-container.md).

One thing the sandbox does not give you is platform coverage. It is Linux with
bash 5; macOS paths and bash 3.2 behaviour need CI's macOS runner or a real Mac.

Read
[`AGENTS.md`](https://github.com/meSingh/polinrider-cleaner/blob/main/AGENTS.md)
first: it covers the layout, the exit-code contract, the release process and the
rules that are not negotiable. The pre-pull-request checklist is in there, and
CI runs the same checks.

Commits and tags are GPG-signed and show as Verified. A repository about
backdated, force-pushed commits that does not sign its own is not credible, so
please do not disable signing.
