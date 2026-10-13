---
title: Every future push
---

> **Being migrated.** The full guide currently lives in
> [`ci/README.md`](https://github.com/meSingh/polinrider-cleaner/blob/main/ci/README.md) and is authoritative until this page
> replaces it.

A scanner you copy into your own repository, so every push is checked with no
third-party action and nothing leaves your infrastructure.

```bash
./ci/install-workflow.sh /path/to/your/repo
```

This vendors the scanner and a workflow into the target repository. It runs on
every push and pull request, and fails the check when a confirmed indicator is
found.

It reads indicators written in `\uXXXX`, `\u{...}` and `\xNN` escapes as well
as plain ones, and reads `package.json` and the three lockfile formats for the
packages they install, so a campaign name counts only as a whole dependency
name. It uses `perl` and `jq` where they are installed, as they are on GitHub's
hosted runners, and says in its output when either is missing.

:::caution[Update a copy vendored from 2.1.0 or earlier]
Those copies match package names as plain substrings and do not decode escape
codes. They miss the escaped loader family and report a project that depends
on a longer package as infected. Re-run the installer with `--force`:

```bash
./ci/install-workflow.sh /path/to/your/repo --force
```
:::

Because the scanner contains the indicator strings it searches for, it will
match itself. The triage filter separates those matches by path; see
[false positives](/reference/false-positives/).
