# Every future push

> **Being migrated.** The full guide currently lives in
> [`ci/README.md`](https://github.com/meSingh/polinrider-cleaner/blob/main/ci/README.md) and is authoritative until this page
> replaces it. Tracked in [ADR-0026](../project/decisions.md).

A scanner you copy into your own repository, so every push is checked with no
third-party action and nothing leaves your infrastructure.

```bash
./ci/install-workflow.sh /path/to/your/repo
```

This vendors the scanner and a workflow into the target repository. It runs on
every push and pull request, and fails the check when a confirmed indicator is
found.

Because the scanner contains the indicator strings it searches for, it will
match itself. The triage filter separates those matches by path; see
[false positives](../reference/false-positives.md).
