---
title: Why it works this way
---

Every design decision that could reasonably have gone the other way is recorded
as an Architecture Decision Record, each with the reasoning **and the cost**.
Start there if you are wondering why something is missing rather than how to
use it.

Records live in
[`docs/adr/`](https://github.com/meSingh/polinrider-cleaner/tree/main/docs/adr)
and are immutable in spirit: a decision that changes gets a new record and the
old one is marked superseded. The history of what was believed, and when, is the
reason to keep them.

## Questions they answer

- [Shell only, with no language runtime](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0001-shell-only-with-no-language-runtime.md) — Accepted
- [Exit code 3 means the scan could not run](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0002-exit-code-3-means-the-scan-could-not-run.md) — Accepted
- [Evidence lives in a temporary directory](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0003-evidence-lives-in-a-temporary-directory.md) — Accepted
- [node_modules is not scanned](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0004-node_modules-is-not-scanned.md) — Accepted
- [Machine checks list recent changes only](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0005-machine-checks-list-recent-changes-only.md) — Accepted
- [Scan roots are confirmed, not guessed](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0006-scan-roots-are-confirmed-not-guessed.md) — Accepted
- [Forks and archived repositories are in scope](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0007-forks-and-archived-repositories-are-in-scope.md) — Accepted
- [The sweep window starts two hours early](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0008-the-sweep-window-starts-two-hours-early.md) — Accepted
- [A known actor escalates rather than dismisses](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0009-a-known-actor-escalates-rather-than-dismisses.md) — Accepted
- [Restore targets are read before they are offered](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0010-restore-targets-are-read-before-they-are-offered.md) — Accepted
- [Pre-attack commits are fetched, not assumed present](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0011-pre-attack-commits-are-fetched-not-assumed-present.md) — Accepted
- [The operator chooses how thorough cleaning is](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0012-the-operator-chooses-how-thorough-cleaning-is.md) — Accepted
- [History rewriting uses filter-repo when it is installed](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0013-history-rewriting-uses-filter-repo-when-it-is-installed.md) — Accepted
- [The presentation layer holds no logic](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0014-the-presentation-layer-holds-no-logic.md) — Accepted
- [No AI attribution in the repository](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0015-no-ai-attribution-in-the-repository.md) — Accepted
- [No external pager](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0016-no-external-pager.md) — Accepted
- [Long operations show git's own progress](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0017-long-operations-show-progress.md) — Accepted
- [Engines hide their standalone guidance when driven](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0018-engines-hide-standalone-guidance-when-driven.md) — Accepted
- [Restore is offered when possible, and explained when it is not](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0019-restore-is-offered-when-possible-and-explained-when-not.md) — Accepted
- [A captured function prints only its value](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0020-a-captured-function-prints-only-its-value.md) — Accepted
- [Only an explicit q leaves a prompt](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0021-only-q-leaves-a-prompt.md) — Superseded by [ADR-0022](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0022-q-quits-b-goes-back.md)
- [q quits, b goes back](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0022-q-quits-b-goes-back.md) — Accepted
- [The entry point parses before it runs](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0023-the-entry-point-parses-before-it-runs.md) — Accepted
- [A prompt inside a loop must not share its stdin](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0024-a-prompt-inside-a-loop-must-not-share-its-stdin.md) — Accepted
- [Walk the filesystem once, with -prune, and checkpoint it](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0025-walk-the-filesystem-once-with-prune-and-checkpoint-it.md) — Accepted
- [2.0.0 is one binary, built against a conformance corpus](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0026-2-0-0-is-one-binary-built-against-a-conformance-corpus.md) — Accepted
- [Development and testing happen in a container](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0027-development-and-testing-happen-in-a-container.md) — Accepted
- [The documentation site is Astro Starlight, on a Node toolchain](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0028-the-documentation-site-is-astro-starlight.md) — Accepted
- [Host state is read through one boundary, and can be supplied](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0029-host-state-is-read-through-one-boundary-and-can-be-supplied.md) — Accepted
- [2.0 does not checkpoint a scan](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0030-2-0-does-not-checkpoint-a-scan.md) — Accepted
- [clean strips an appended payload in place, and never touches git](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0031-clean-strips-an-appended-payload-in-place-and-never-touches-git.md) — Accepted
- [The guided flow changes something only on a typed yes](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0032-the-guided-flow-changes-something-only-on-a-typed-yes.md) — Accepted
- [Every run opens with the banner, and colour changes no character](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0033-every-run-opens-with-the-banner-and-colour-changes-no-character.md) — Accepted
- [The verdict is followed by what to do, and "rebuild" is said only on proof](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0034-the-verdict-is-followed-by-what-to-do-and-rebuild-is-said-only-on-proof.md) — Accepted
- [The guided flow is four calm screens, answered in words](https://github.com/meSingh/polinrider-cleaner/blob/main/docs/adr/0035-the-guided-flow-is-four-calm-screens-answered-in-words.md) — Accepted
