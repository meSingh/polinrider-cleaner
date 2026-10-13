# Continuous scanning

<sub>[← back to the main README](../README.md) · this folder is for **preventing
the next one**. It is not part of cleaning up an active incident.</sub>

---

**What this does.** Scans every push and pull request in **your own**
repositories, using a copy of the scanner that lives inside them.

**Why a copy.** So the scan does not depend on anything that can change under
you. See [below](#why-the-scanner-is-vendored-not-installed).

| File | What it is |
|---|---|
| `scan-workspace.sh` | The scanner. Scans a working tree, and with `--all-refs` every ref in the repository |
| `polinrider-scan.yml` | The workflow that runs it. Copy into `.github/workflows/` |
| `install-workflow.sh` | Copies both, plus the indicator set, into one of your repositories |
| `selftest.sh` | Builds a synthetic infected repo and a clean control, and asserts the scanner gets both right |

---

## Why the scanner is vendored, not installed

A scan step that pulls a third-party action on every push is the same shape as the
attack it is meant to catch: someone else's code, fetched automatically, running
with your repository's context. If that action's publisher is compromised, and that
is exactly what is happening across npm, Go, Composer and the VS Code marketplace
right now, your detection tooling becomes the delivery mechanism.

So `install-workflow.sh` copies the scanner into `.github/polinrider/` in your
repository. After that the scan runs entirely from code you control, reviewed in
your own pull requests, with no network fetch at scan time.

The only action used is `actions/checkout`, published by GitHub, pinned to a
commit SHA rather than a moving tag.

---

## Install

See exactly what it will do first. Nothing is written:

```bash
./install-workflow.sh /path/to/your/repo --dry-run
```

```
DRY RUN. Nothing will be written.

Target repository: /path/to/your/repo

Files:
  .github/workflows/polinrider-scan.yml                the workflow itself
  .github/polinrider/scan-workspace.sh                 the scanner, executable
  .github/polinrider/ioc/bad-packages.txt              indicator data
  .github/polinrider/ioc/filenames.txt                 indicator data
  .github/polinrider/ioc/hashes.txt                    indicator data
  .github/polinrider/ioc/implant-names.txt             indicator data
  .github/polinrider/ioc/implant-paths.txt             indicator data
  .github/polinrider/ioc/network.txt                   indicator data
  .github/polinrider/ioc/strong.txt                    indicator data
  .github/polinrider/ioc/weak.txt                      indicator data

Total: 10 files, about 64 KB. No dependencies are added.
```

Then run it for real:

```bash
./install-workflow.sh /path/to/your/repo
```

### What lands in your repository

```
.github/
├── workflows/
│   └── polinrider-scan.yml      the workflow. Reads nothing from outside your repo
└── polinrider/
    ├── scan-workspace.sh        the scanner, ~600 lines of shell you can read
    └── ioc/
        └── *.txt                the indicator set, plain text, one entry per line
```

Ten files, about 64 KB. **No dependency is added to your project.** Nothing is
downloaded at scan time. No `package.json`, lockfile, submodule or container
image is touched. If you delete `.github/polinrider/` and the workflow file, the
installation is completely gone.

### What it does not do

- It does not commit. You review the diff and commit yourself.
- It does not overwrite an existing `polinrider-scan.yml` unless you pass
  `--force`. A dry run tells you if one is already there.
- It does not modify any file you already had.
- It does not phone home, and neither does the workflow it installs.

### Before you commit

Run the scanner locally, so any false positive shows up on your machine rather
than as a red X on your next pull request:

```bash
cd /path/to/your/repo
.github/polinrider/scan-workspace.sh --path . --all-refs
```

Expected on a clean repository:

```
=========================================
 PolinRider workspace scan
   infected findings : 0
   review findings   : 0
=========================================
```

If it reports something, read
[Tuning](#tuning) before you commit anything.

Then:

```bash
git add .github/polinrider .github/workflows/polinrider-scan.yml
git commit -m "Add PolinRider scan workflow"
```

### Keeping it current

Signatures rotate. When `ioc/*.txt` changes in this repository, re-run the
installer against each repo that has the vendored copy:

```bash
./install-workflow.sh /path/to/your/repo --force
```

> [!IMPORTANT]
> **Vendored a copy from 2.1.0 or earlier? Update it.** Those copies match
> package names as plain substrings and do not decode escape codes, so they
> miss the loader family written in `\uXXXX` escapes and report a project that
> depends on a longer package (`sass-formatter` for `sass-format`) as infected.
> Re-run the installer with `--force` as above.

### Removing it

```bash
rm -rf /path/to/your/repo/.github/polinrider \
       /path/to/your/repo/.github/workflows/polinrider-scan.yml
```

That is the whole uninstall.

## What the workflow does

Runs on every push, every pull request, manually, and weekly on a schedule.
Reinfection with a rotated signature is documented behaviour for this campaign,
so scanning only on push is not enough.

It checks out with `fetch-depth: 0` and `persist-credentials: false`: full
history so `--all-refs` can see it, and no token left in the runner's git config
for a payload to find.

Findings appear as GitHub annotations on the run. `INFECTED` fails the job;
`review` does not.

---

## What it checks

In the working tree, and with `--all-refs` in every branch and tag:

- **`strong.txt`**, as fixed strings.
- **The same indicators written in escape codes.** Files holding `\uXXXX`,
  `\u{...}` or `\xNN` are decoded and matched again, because one loader family
  is written entirely in them
  ([ADR-0040](../docs/adr/0040-indicators-are-matched-through-escape-codes-as-well.md)).
  Only files with an escape are read twice. In history each file version is
  decoded once, however many refs carry it.
- **Campaign package names, compared whole**
  ([ADR-0041](../docs/adr/0041-package-names-are-read-from-manifests-and-compared-whole.md)).
  `package.json`, `package-lock.json`, `yarn.lock` and `pnpm-lock.yaml` are read
  for the packages they install, so a name in a description is not a
  dependency. Anywhere else a name counts only where it stands on its own, so
  `sass-format` is not found in `sass-formatter` or under `@other/`. A manifest
  that cannot be read is matched as text and gets a review line saying so.
- In the working tree only: `weak.txt` (review), indicator filenames, a
  `tasks.json` that runs on folder open, fonts that are not fonts, and build
  configs with a payload after the module end.

The lines the two newer checks print:

```
INFECTED  path/postcss.config.mjs: contains an indicator once its escape codes are decoded
INFECTED  path/yarn.lock: known-bad package referenced: <name>
review    path/package.json: could not be read as a manifest, so package names were matched as text
```

### What it needs

bash 3.2 or later, git and standard POSIX tools. Two more are used where they
are installed. GitHub's hosted runners carry both, and perl also ships with
macOS and Git for Windows:

| Tool | Used for | Without it |
|---|---|---|
| `perl` | decoding escape codes (core perl only, no modules) | not decoded, and a review line counts the files that were not: `N files hold escape codes and were not decoded because perl is not installed, ...` |
| `jq` | reading `package.json` and `package-lock.json` | those two are matched as text, as before. A note says so, and any that names a campaign package gets a review line beside the finding |

`yarn.lock` and `pnpm-lock.yaml` are read with awk and need neither.

Where it still differs from the binary: names in an unreadable manifest are
matched as written, not decoded; `jq` keeps the last of two duplicate keys
where the binary's parser keeps every one; and the history pass reads only
files that contain a campaign name, so an unreadable manifest that names none
is reported in the working tree but not in history.

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

> [!NOTE]
> **Your own security tooling gets flagged.** Any file that searches for these
> indicators contains them by definition.

Exclude it by path:

```bash
./scan-workspace.sh --path . --exclude '(^|/)security/scanners/'
```

> [!WARNING]
> **A finding you believe is wrong?** Do not widen the exclusions until you have
> read the matched line. Then
> [open a false-positive issue](../.github/ISSUE_TEMPLATE/false-positive.md) so
> the fix reaches everyone rather than only your repository.

**Keeping indicators current.** Signatures rotate. Update [`../ioc/`](../ioc/),
re-run `install-workflow.sh` on each repository, and re-run `selftest.sh` before
you trust the change:

```bash
./selftest.sh
```

It plants one artifact of each class in a synthetic repository, asserts all of
them are found, then asserts a clean control with the four known false-positive
shapes produces no findings at all. It then runs the escape-code and
package-name cases of the conformance corpus against the shell scanner, with
and without `perl` and `jq`, and an escaped payload left only on an older ref.
