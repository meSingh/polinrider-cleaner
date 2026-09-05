# Contributing

People run this during an incident, on machines they no longer trust, against
repositories they cannot afford to lose. That sets the bar.

## Rules

1. **Nothing is deleted.** Destructive actions are reversible: quarantine moves
   files, restore moves a branch pointer to a commit that still exists. If you
   cannot make something reversible, print the command and let the operator run it.
2. **Dry run by default.** Anything that changes state needs `--apply`.
3. **No new dependencies.** `git`, `gh`, `jq`, and POSIX shell tools. No Node, no
   Python, no packages. Windows gets PowerShell 5.1, which ships with the OS.
4. **A false `INFECTED` is worse than a missed `review`.** Someone acting on a
   false positive during an incident burns hours they do not have. If a legitimate
   project could contain the string, it belongs in `weak.txt`.
5. **Read the whole script before changing it.** Every heuristic in here exists
   because of a specific false positive or a specific miss during a real cleanup.

## Before you open a pull request

```bash
bash -n <every script you touched>
shellcheck --severity=warning --external-sources <every script you touched>
./ci/selftest.sh            # detection: infected fixture and clean control
./ci/selftest-restore.sh    # restore planner: classification and the wave-2 trap
./ci/selftest-implant.sh    # implant detection, hashing and quarantine
./ci/selftest-entrypoint.sh # polinrider.sh routing, exit codes and read-only behaviour
```

The tree is clean at `--severity=warning`, which is what CI enforces. Where a
warning is deliberately suppressed there is a `# shellcheck disable=` with the
reason on the line above it.

For `machine-cleanup/check-windows.ps1`, CI parses it and runs PSScriptAnalyzer.
Both must be clean.

`selftest.sh` builds a synthetic infected repository and a clean control
containing the known false-positive shapes, and asserts the scanner gets both
right. **Add a case to it for any detection you add or fix.**

`selftest-restore.sh` covers the only code path that can destroy someone's work.
It builds a two-wave attack offline and asserts that the planner refuses to
restore onto an attacker commit. **Any change to `lib/gh-restore.sh` needs a case
here.** It makes no network calls, so it is safe to run anywhere.

## Adding an indicator

See [`ioc/README.md`](ioc/README.md). Link a public source. Do not commit live
payload code, obfuscated or not — an indicator string, a hash, a package name or
a hostname is enough.

## Compatibility

Scripts must run on bash 3.2, which is what macOS ships. No `mapfile`, no
associative arrays, no `${var,,}`. Guard array expansion under `set -u`:
`${arr[@]+"${arr[@]}"}`.

## Conduct

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). The short version: people arrive
here mid-incident. Answer the question they asked.

## Signing

Commits and tags here are GPG-signed. If you are opening a pull request, signing
yours is appreciated but not required — the maintainer's signature on the merge
is what the release provenance chains to.

```bash
git config commit.gpgsign true
git config tag.gpgsign true
```

## Reporting

Use the issue templates. False positives and missed detections are the two most
useful reports. Include the exact command, the exact output, and your environment.
