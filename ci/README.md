# Continuous scanning

Scan every push and pull request for PolinRider artifacts, using code that lives
inside your own repository.

| File | What it is |
|---|---|
| `scan-workspace.sh` | The scanner. Scans a working tree, and with `--all-refs` every ref in the repository |
| `polinrider-scan.yml` | The workflow that runs it. Copy into `.github/workflows/` |
| `install-workflow.sh` | Copies both, plus the indicator set, into one of your repositories |
| `selftest.sh` | Builds a synthetic infected repo and a clean control, and asserts the scanner gets both right |
| `selftest-restore.sh` | Offline test of the restore planner, including the second-wave trap. No network, no credentials |
| `selftest-implant.sh` | Offline test of second-stage implant detection and quarantine, in a throwaway HOME |

---

## Why the scanner is vendored, not installed

A scan step that pulls a third-party action on every push is the same shape as the
attack it is meant to catch: someone else's code, fetched automatically, running
with your repository's context. If that action's publisher is compromised — the
exact thing happening across npm, Go, Composer and the VS Code marketplace right
now — your detection tooling becomes the delivery mechanism.

So `install-workflow.sh` copies the scanner into `.github/polinrider/` in your
repository. After that the scan runs entirely from code you control, reviewed in
your own pull requests, with no network fetch at scan time.

The only action used is `actions/checkout`, published by GitHub, pinned to a
commit SHA rather than a moving tag.

---

## Install

```bash
./install-workflow.sh /path/to/your/repo
```

That writes three things and commits nothing:

```
.github/polinrider/scan-workspace.sh
.github/polinrider/ioc/*.txt
.github/workflows/polinrider-scan.yml
```

Verify it passes before you commit, so you find any false positive locally rather
than in CI:

```bash
cd /path/to/your/repo
.github/polinrider/scan-workspace.sh --path . --all-refs
git add .github/polinrider .github/workflows/polinrider-scan.yml
git commit -m "Add PolinRider scan workflow"
```

Re-run the installer after you update `ioc/*.txt` here, so the vendored copies
stay current.

---

## What the workflow does

Runs on every push, every pull request, manually, and weekly on a schedule.
Reinfection with a rotated signature is documented behaviour for this campaign,
so scanning only on push is not enough.

It checks out with `fetch-depth: 0` and `persist-credentials: false` — full
history so `--all-refs` can see it, and no token left in the runner's git config
for a payload to find.

Findings appear as GitHub annotations on the run. `INFECTED` fails the job;
`review` does not.

---

## Running it directly

```bash
./scan-workspace.sh [--path DIR] [--all-refs] [--fail-on LEVEL] [--exclude RE]
```

| Option | Default | Meaning |
|---|---|---|
| `--path DIR` | `.` | Directory to scan |
| `--all-refs` | off | Also scan every git ref, not just the working tree. Catches a payload that was committed and then reverted |
| `--fail-on LEVEL` | `infected` | `infected`, `review`, or `never` |
| `--exclude RE` | see below | Extended regex of paths to skip. Repeatable |
| `--scan-docs` | off | Also scan `.md` files |
| `--ioc DIR` | auto | Indicator directory |

Exit codes: `0` clean, `1` review findings (only with `--fail-on review`), `2`
infected findings.

Skipped by default: `.git/`, `node_modules/`, `.github/polinrider/`, `ioc/`, any
`.github/workflows/*polinrider*.yml`, and every `.md` file.

The last four are the same problem in different clothes: detection tooling and
documentation contain the indicator strings they are about. Markdown also cannot
execute, so scanning it buys nothing. Pass `--scan-docs` if you want it anyway.

---

## Tuning

**Your own security tooling gets flagged.** Any file that searches for these
indicators contains them. Exclude it by path:

```bash
./scan-workspace.sh --path . --exclude '(^|/)security/scanners/'
```

**A finding you believe is wrong.** Do not widen the exclusions until you have
read the matched line. Then
[open a false-positive issue](../.github/ISSUE_TEMPLATE/false-positive.md) so the
fix reaches everyone rather than only your repository.

**Keeping indicators current.** Signatures rotate. Update [`../ioc/`](../ioc/),
re-run `install-workflow.sh` on each repository, and re-run `selftest.sh` before
you trust the change:

```bash
./selftest.sh
```

It plants one artifact of each class in a synthetic repository, asserts all of
them are found, then asserts a clean control with the four known false-positive
shapes produces no findings at all.
