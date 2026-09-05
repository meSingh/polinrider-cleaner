# AGENTS.md

Instructions for AI coding agents. Humans want [README.md](README.md).

This file follows the [AGENTS.md](https://agents.md/) convention. It has two
audiences, and the second one is unusual, so read both headings before acting.

---

## If you were asked to CLEAN UP an infection with these tools

You are operating an incident-response tool on someone's real repositories and
real machine. Read this section completely before running anything.

### Non-negotiable order

```
1. machine-cleanup/     on every affected machine
2. credential rotation                       (the human does this, not you)
3. github-org-recovery/ or github-account-recovery/         restore the branches
4. ci/                                       prevent the next one
```

Restoring branches while an infected machine still holds a valid token means the
attacker re-pushes within minutes. This is documented behaviour of this
campaign, not a theoretical risk. **Never reorder these.**

### Start here

`./polinrider.sh` is the entry point. It works out what needs scanning, picks the
right tool for the operating system it is on, and prints the next command. Every
mode is read-only. Pass `--yes` so it never prompts:

```bash
./polinrider.sh --machine --yes
./polinrider.sh --org ACME --yes
./polinrider.sh --path /some/repo --yes
```

Exit codes: `0` nothing found, `1` review items only, `2` something confirmed.

### What you may run without asking

Everything below is read-only. It writes to an output directory and changes
nothing on GitHub or on the machine.

| Command | Effect |
|---|---|
| `polinrider.sh` in any mode | routes to the tools below; never changes anything |
| `ci/selftest*.sh` (four of them) | offline, no network, no credentials |
| `ci/scan-workspace.sh --path DIR` | reads files |
| `machine-cleanup/check-*.sh DIR ...` | reads the machine, writes one report file |
| `github-org-recovery/scan.sh`, `sweep.sh`, `triage-filter.sh`, `preflight.sh` | reads the GitHub API, clones mirrors |
| `github-account-recovery/*` (same four) | as above |
| `restore.sh` **without** `--apply` | prints a plan, changes nothing |

### What you must NOT run without explicit human confirmation

| Command | Why |
|---|---|
| `restore.sh --apply` | force-updates branch refs on GitHub. Irreversible from the tool's side |
| `check-*.sh --apply` / `-Apply` | moves files on the human's machine |
| Any `gh api -X DELETE` or `-X PATCH` printed by these tools | the tools print commands deliberately so a human runs them |

For `restore.sh --apply` specifically, all of these must be true first, and you
must confirm them rather than assume them:

1. `preflight.sh` exits 0.
2. The compromised account's access is actually severed. The human confirms this;
   no API call proves it.
3. Every row in `restore-plan.tsv` is a push nobody on the team claims. Ask.
4. No row is marked `MALICIOUS_TARGET`. If any is, the `--since` window starts
   too late. Widen it and rebuild the plan.

### How to read the output

- **Read what matched, never the count.** `cat evidence/triage.txt`. An
  `INFECTED` count is meaningless on its own.
- **Run `triage-filter.sh` before drawing conclusions.** A grep-based scanner
  flags its own detection rules. Findings in files that *detect* this malware are
  expected and are not infections.
- **`ok_orphaned` in a restore plan is normal and good.** It means the target
  commit is unreachable from any ref, so the mirror never fetched it, but GitHub
  still holds it and confirmed so. It does not mean work was lost.
- **`clean` means the current indicator set is absent from that ref.** It is not
  proof the ref was never touched.
- Exit codes for the local checks: `0` clean, `1` review items only, `2` a
  confirmed indicator.

### Things that are true and counter-intuitive

- **Do not `git pull` into an existing clone of an affected repository.** Delete
  the clone and re-clone after the remote is verified clean. A pull into an
  infected clone re-infects the remote.
- **Do not read git history to decide whether a branch is clean.** The
  propagation script backdates its commits, so `git log` shows nothing wrong.
- **Evidence capture is time-critical.** Pre-attack commit SHAs come from an
  events API that keeps roughly the last 300 events per repository. Every push
  anyone makes pushes the attacker's push closer to falling off the end. Capture
  before you clean.
- **A quarantined LaunchAgent or systemd unit is still running.** Moving the file
  does not stop the process it started. The tools print the command that does;
  surface it to the human.

### Never do these

- Never invent an indicator. Everything in `ioc/` traces to a named public
  source. A false `INFECTED` costs someone hours during an incident.
- Never widen `--exclude` to make a finding go away before a human has read the
  matched line.
- Never paste the human's report file into a public issue. It contains paths
  from their machine.

---

## If you were asked to CHANGE this repository

### What it is

Shell and PowerShell only. No Node, no Python, no package manager, no build
step. Dependencies are `git`, `gh` and `jq`; the local checks and the CI scanner
need only `git` and POSIX tools. Windows uses PowerShell 5.1, which ships with
the OS.

### Layout

| Path | Contains |
|---|---|
| `ioc/` | the indicator set, single source of truth, read at runtime by everything |
| `lib/` | shared engines: `gh-scan`, `gh-sweep`, `gh-restore`, `triage-filter`, `common.sh`, `local-common.sh` |
| `github-org-recovery/` | recover a GitHub **organization**: thin wrappers over `lib/` plus its own `preflight.sh` |
| `github-account-recovery/` | the same for **one personal account** |
| `machine-cleanup/` | check and clean **one computer**, one script per operating system |
| `polinrider.sh` | the single entry point at the repository root |
| `ci/` | the vendorable scanner, its workflow template, installer, and three self-tests |

`common.sh` and `local-common.sh` are sourced, not executed, and are
deliberately not marked executable.

### Before you open a pull request

```bash
bash -n <every script you touched>
shellcheck --severity=warning --external-sources <every script you touched>
./ci/selftest.sh
./ci/selftest-restore.sh
./ci/selftest-implant.sh
```

CI enforces `shellcheck --severity=warning` and runs all three self-tests, plus
a PowerShell parse and PSScriptAnalyzer pass at Error and Warning severity.
The tree is clean at those levels; keep it that way. Where a warning is
suppressed there is a `# shellcheck disable=` with the reason on the line above.

### Rules that are not negotiable

1. **Nothing is deleted, ever.** Quarantine moves files and writes a manifest.
   Restore moves a branch pointer to a commit that still exists.
2. **Dry run by default.** Anything that changes state requires `--apply`.
3. **No new dependencies.**
4. **A false `INFECTED` is worse than a missed `review`.** If a legitimate
   project could contain the string, it belongs in `ioc/weak.txt`.
5. **Never match untrusted content against the full command line or the full
   output line.** Match the specific field. Two real bugs came from this: a
   scanner that excluded nothing because it tested `<ref>:<path>:<line>` against
   a path regex, and an implant check that reported the operator's own shell.
6. **Strip control characters from anything derived from a scanned file** before
   it reaches a terminal or a report. A crafted filename or file header would
   otherwise drive the operator's terminal.
7. **Add a self-test case for any detection you add or fix.**

### Compatibility

Scripts must run on bash 3.2, which is what macOS ships. No `mapfile`, no
associative arrays, no `${var,,}`. Guard array expansion under `set -u` with
`${arr[@]+"${arr[@]}"}`.

### Cutting a release

The README tells people to clone a **specific tag**, not `main` and not `latest`.
That pin has to be updated by hand when a new release goes out, or the front page
keeps pointing at an old version. This is the checklist.

**1. The version appears in exactly one file.** `README.md`, in the "Run it"
section: the `git clone --branch vX.Y.Z` line, the two sentences below it that
name the tag, and the `git verify-tag vX.Y.Z` line. Four occurrences, one block.
Check with:

```bash
grep -rn 'v[0-9]\+\.[0-9]\+\.[0-9]\+' README.md
```

**2. Merge everything first.** `main` is protected: no direct pushes, four
required checks, and the rule applies to admins. Every change goes through a
pull request.

**3. Tag from `main`, signed.**

```bash
git checkout main && git pull
git tag -s vX.Y.Z -m "polinrider-cleaner vX.Y.Z

<what changed and why it matters to someone running this>"
git push origin vX.Y.Z
```

`tag.gpgsign` is enabled, so `-s` is the default; keep it explicit anyway.

**4. The Release workflow does the rest.** It re-runs all four self-tests at that
tag, builds the archive, records SHA-256, attaches a sigstore provenance bundle,
and publishes. If the tests fail, no release is created, which is intentional.

**5. Update the pin in the README** and open a pull request for it. The pin
cannot be updated before the tag exists, so this is always a follow-up commit.

> Tags matching `refs/tags/v*` are protected by a repository ruleset with no
> bypass actors: they cannot be updated, force-pushed or deleted by anyone,
> including the maintainer. A published tag is permanent. If you tagged the wrong
> commit, cut the next patch version; do not try to move the tag.

### Signing

Commits and tags in this repository are GPG-signed and show as Verified on
GitHub. If you are committing on the maintainer's behalf, do not disable
signing, and do not add `--no-gpg-sign`. A repository about backdated,
force-pushed commits that does not sign its own is not credible.

### Commits and pull requests

Explain why the change is needed before what it does. Templates are in
`.github/`. Do not add AI attribution or co-author trailers.
