# Documentation

| If you want to | Read |
|---|---|
| clean up an incident, start to finish | [`../README.md`](../README.md) |
| recover a personal GitHub account | [the account guide](https://mesingh.github.io/polinrider-cleaner/guides/account/) |
| recover an organization | [the organization guide](https://mesingh.github.io/polinrider-cleaner/guides/organization/) |
| check or clean one computer | [the machine guide](https://mesingh.github.io/polinrider-cleaner/guides/machine/) |
| run the scanner in CI | [`../ci/README.md`](../ci/README.md) |
| understand the indicator set | [`../ioc/README.md`](../ioc/README.md) |
| know when the indicator set was last reviewed | [the indicator reviews](https://mesingh.github.io/polinrider-cleaner/indicator-reviews/) |
| change this repository, or point an agent at it | [`../AGENTS.md`](../AGENTS.md) |
| test a change, or try the tool safely | [`../CONTRIBUTING.md`](../CONTRIBUTING.md) |
| know **why** it works the way it does | [`adr/`](./adr/) |
| know what this tool does not promise | [`../DISCLAIMER.md`](../DISCLAIMER.md) |

## Why it works this way

[`adr/`](./adr/) holds one record per design decision that could reasonably have
gone the other way, each with the reasoning **and the cost**.

Start there if you are wondering why something is missing rather than how to use
it. Common questions it answers:

- [Why is `node_modules` not scanned?](./adr/0004-node_modules-is-not-scanned.md)
- [Why does a colleague's name on a hostile push not clear them?](./adr/0009-a-known-actor-escalates-rather-than-dismisses.md)
- [Why does rewriting history not remove anything from GitHub?](./adr/0013-history-rewriting-uses-filter-repo-when-it-is-installed.md)
- [Why is the evidence directory deleted on restart?](./adr/0003-evidence-lives-in-a-temporary-directory.md)
- [Why does a failed scan not report as clean?](./adr/0002-exit-code-3-means-the-scan-could-not-run.md)

## Images

`img/` holds the artwork used by the README. Nothing reads it at runtime.
