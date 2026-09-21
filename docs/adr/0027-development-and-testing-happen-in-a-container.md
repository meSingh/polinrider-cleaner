# 0027. Development and testing happen in a container

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-21 |

## Context

This tool moves files into quarantine, rewrites git history and walks `$HOME`.
Its test suite exercises exactly those paths. Every run of `ci/selftest-*.sh`
and `conformance/run.sh` so far has executed against the maintainer's real
filesystem, with nothing between a bad path expansion and their actual files.

The suite is careful — fixtures are built in `mktemp -d` and `$HOME` is
overridden per test — but careful is a property of the code as written, not a
property enforced by anything. A regression in the one place that computes a
destination path is enough, and that is precisely the code most under change
during the 2.0.0 port.

## Decision

`ci/sandbox.sh` runs any command in this repository inside a container built
from `.devcontainer/Dockerfile`.

- `$HOME` is the container's, so a run that walks or writes `$HOME` cannot
  reach the developer's.
- The repository is mounted **read-only** at `/work`. A test cannot modify the
  source it is testing, and cannot corrupt the git history.
- Build output goes to a named volume under `$HOME/build`, so cargo works
  against a read-only checkout. Deliberately not at `$HOME` itself, which would
  mask the image's own git configuration.
- **Networking is off** unless `--net` is passed. A scan has no business
  opening a connection, and this is how that stops being a promise.
- Non-root, `--cap-drop ALL`, `no-new-privileges`, a pid limit.
- The base image is pinned by digest. A supply-chain cleanup tool that pulls a
  floating base image is not making an argument it can defend.

`./ci/sandbox.sh --all` runs lint, every self-test and the conformance corpus.

## Consequences

Better: the destructive paths are exercised somewhere they cannot do damage,
and a contributor gets the same environment without being asked to trust a
tool that quarantines files with their home directory attached.

The sandbox also found a real bug on its first full run, which is the strongest
argument for it. `selftest-implant` failed intermittently, and only inside the
container. The cause was not the container. The state directory introduced in
ADR-0025 is named `polinrider-scan-<timestamp>` at second resolution, and that
test runs the check twice in quick succession. When both landed in the same
second they shared a state directory, so the second run found every check
recorded as `done` and skipped all of them: no findings, no quarantine, and a
clean verdict for work it never did. A run without `--resume` now clears stale
checkpoints. Silently reporting clean is the worst failure this tool has, and
it took a different environment to expose it.

Worse, and worth stating:

- **Isolation is not platform coverage.** This is Linux with bash 5. macOS
  paths and bash 3.2 behaviour are exercised only by CI's macOS runner and by
  running on a real Mac. Passing in the sandbox does not mean passing on the
  platform most of this tool's users are on.
- **Docker is now a development dependency.** It is not a runtime dependency
  and must never become one; the tool itself still has to run on a bare machine
  mid-incident with nothing installed.
- **A read-only mount makes some workflows awkward.** Anything that writes into
  the repository has to be run on the host or given an explicit writable mount.
- **The container hides host-specific faults.** Two self-tests failed in the
  sandbox for reasons that were the sandbox's fault rather than the code's: a
  missing `gh`, and an `init.defaultBranch` set in the image that made a
  fixture look for a branch it never created. An environment that differs from
  the host finds real bugs and manufactures fake ones, and telling them apart
  is work.
- **Docker Desktop on macOS misreports permissions.** `[[ -x ]]` returns true
  for a mode-644 file on a bind mount. `ci/selftest-ui.sh` now asks git for the
  committed mode instead, which is the more correct question anyway.

## Related

- ADR-0025, the state directory this found the bug in.
- ADR-0026, the 2.0.0 port; the sandbox exists mainly to make that port safe to
  iterate on.
