# 0013. History rewriting uses filter-repo when it is installed

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

`git filter-repo` is roughly two orders of magnitude faster than `git filter-branch` and is what git itself recommends. It is also Python, which [ADR-0001](./0001-shell-only-with-no-language-runtime.md) excludes as a dependency.

## Decision

`filter-repo` is used when already present, `filter-branch` otherwise. Nobody is asked to install anything. `PRC_REWRITE_BACKEND` forces either, for testing or for an operator who would rather not use `filter-repo`.

## Consequences

Two code paths to maintain, and `ci/selftest-rewrite.sh` runs the same fixture through both.

They are not identical. `filter-branch` leaves `refs/original/*`, which meant a post-rewrite assertion counting over `--all` saw the payload and refused to push a successful rewrite.

**Neither removes anything from GitHub.** GitHub keeps unreachable objects and serves them by SHA, which is how [ADR-0011](./0011-pre-attack-commits-are-fetched-not-assumed-present.md) works. After a rewrite and force-push, anyone holding an old commit id can still fetch it. Purging means asking GitHub Support to run `gc`; forks keep their own copies regardless. The dry run says this before `--apply`.
