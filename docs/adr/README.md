# Architecture decision records

Each file records one choice that could reasonably have gone the other way,
with the reasoning **and the cost**.

An entry that only says why something is good is not worth writing. Every record
here names what the decision makes worse, because that is the part you need when
you are deciding whether to change it.

If you disagree with one, open an issue and quote its number.

> If a record and the code disagree, **the code is the truth and the record is a
> bug.** Say so in an issue rather than assuming the code is wrong.

## The records

| # | Decision | Status |
|---|---|---|
| [0001](./0001-shell-only-with-no-language-runtime.md) | Shell only, with no language runtime | Accepted |
| [0002](./0002-exit-code-3-means-the-scan-could-not-run.md) | Exit code 3 means the scan could not run | Accepted |
| [0003](./0003-evidence-lives-in-a-temporary-directory.md) | Evidence lives in a temporary directory | Accepted |
| [0004](./0004-node_modules-is-not-scanned.md) | node_modules is not scanned | Accepted |
| [0005](./0005-machine-checks-list-recent-changes-only.md) | Machine checks list recent changes only | Accepted |
| [0006](./0006-scan-roots-are-confirmed-not-guessed.md) | Scan roots are confirmed, not guessed | Accepted |
| [0007](./0007-forks-and-archived-repositories-are-in-scope.md) | Forks and archived repositories are in scope | Accepted |
| [0008](./0008-the-sweep-window-starts-two-hours-early.md) | The sweep window starts two hours early | Accepted |
| [0009](./0009-a-known-actor-escalates-rather-than-dismisses.md) | A known actor escalates rather than dismisses | Accepted |
| [0010](./0010-restore-targets-are-read-before-they-are-offered.md) | Restore targets are read before they are offered | Accepted |
| [0011](./0011-pre-attack-commits-are-fetched-not-assumed-present.md) | Pre-attack commits are fetched, not assumed present | Accepted |
| [0012](./0012-the-operator-chooses-how-thorough-cleaning-is.md) | The operator chooses how thorough cleaning is | Accepted |
| [0013](./0013-history-rewriting-uses-filter-repo-when-it-is-installed.md) | History rewriting uses filter-repo when it is installed | Accepted |
| [0014](./0014-the-presentation-layer-holds-no-logic.md) | The presentation layer holds no logic | Accepted |
| [0015](./0015-no-ai-attribution-in-the-repository.md) | No AI attribution in the repository | Accepted |
| [0016](./0016-no-external-pager.md) | No external pager | Accepted |
| [0017](./0017-long-operations-show-progress.md) | Long operations show git's own progress | Accepted |
| [0018](./0018-engines-hide-standalone-guidance-when-driven.md) | Engines hide their standalone guidance when driven | Accepted |
| [0019](./0019-restore-is-offered-when-possible-and-explained-when-not.md) | Restore is offered when possible, and explained when it is not | Accepted |
| [0020](./0020-a-captured-function-prints-only-its-value.md) | A captured function prints only its value | Accepted |
| [0021](./0021-only-q-leaves-a-prompt.md) | Only an explicit q leaves a prompt | Superseded by [ADR-0022](./0022-q-quits-b-goes-back.md) |
| [0022](./0022-q-quits-b-goes-back.md) | q quits, b goes back | Accepted |
| [0023](./0023-the-entry-point-parses-before-it-runs.md) | The entry point parses before it runs | Accepted |
| [0024](./0024-a-prompt-inside-a-loop-must-not-share-its-stdin.md) | A prompt inside a loop must not share its stdin | Accepted |

## Adding one

Copy [`template.md`](./template.md), take the next number, and link it from the
table above. Change an existing record only to correct it; if the decision
itself changes, write a new record and set the old one to
`Superseded by ADR-XXXX`.

Records are immutable in spirit. The history of what was believed, and when, is
the reason to keep them.
