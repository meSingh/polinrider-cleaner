# Contributing

The most valuable contributions to this project are not code.

## An indicator we are missing

[Open a missed-detection issue](https://github.com/meSingh/polinrider-cleaner/issues/new?template=missed-detection.md)
with the source you saw it in. New indicators are reviewed against the
classification rule on [the indicator set](../campaign/indicators.md): `strong.txt`
only when a match means infection with no plausible alternative.

## A false positive

[Open a false-positive issue](https://github.com/meSingh/polinrider-cleaner/issues/new?template=false-positive.md).
These matter more than they look. A tool that cries wolf during an incident
wastes the one resource nobody has, and the
[known ones](../reference/false-positives.md) were all found this way.

## A fix to these docs

Every page has an edit link in the top right. It opens the Markdown source in
GitHub's editor and turns your change into a pull request. No local setup, no
toolchain.

## Code

Read
[`AGENTS.md`](https://github.com/meSingh/polinrider-cleaner/blob/main/AGENTS.md)
first: it covers the layout, the exit-code contract, the release process and the
rules that are not negotiable. The pre-pull-request checklist is in there, and
CI runs the same checks.

Commits and tags are GPG-signed and show as Verified. A repository about
backdated, force-pushed commits that does not sign its own is not credible, so
please do not disable signing.
