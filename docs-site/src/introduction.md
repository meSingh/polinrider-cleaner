# polinrider-cleaner

<ul class="badges">
<li><a href="https://github.com/meSingh/polinrider-cleaner/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/meSingh/polinrider-cleaner/actions/workflows/ci.yml/badge.svg"></a></li>
<li><a href="https://scorecard.dev/viewer/?uri=github.com/meSingh/polinrider-cleaner"><img alt="OpenSSF Scorecard" src="https://api.scorecard.dev/projects/github.com/meSingh/polinrider-cleaner/badge"></a></li>
<li><a href="https://github.com/meSingh/polinrider-cleaner/actions/workflows/codeql.yml"><img alt="CodeQL" src="https://github.com/meSingh/polinrider-cleaner/actions/workflows/codeql.yml/badge.svg"></a></li>
<li><a href="https://github.com/meSingh/polinrider-cleaner/actions/workflows/semgrep.yml"><img alt="Semgrep" src="https://github.com/meSingh/polinrider-cleaner/actions/workflows/semgrep.yml/badge.svg"></a></li>
<li><a href="https://github.com/meSingh/polinrider-cleaner/commits/main"><img alt="Commits are GPG signed" src="https://img.shields.io/badge/commits-GPG%20signed-2c7048"></a></li>
<li><a href="https://github.com/meSingh/polinrider-cleaner/releases/latest"><img alt="Releases carry build provenance" src="https://img.shields.io/badge/releases-attested%20provenance-2c7048"></a></li>
<li><a href="https://github.com/meSingh/polinrider-cleaner/tree/main/docs/indicator-reviews"><img alt="Indicators reviewed weekly" src="https://img.shields.io/badge/indicators-reviewed%20weekly-0f6d63"></a></li>
<li><img alt="Zero runtime dependencies" src="https://img.shields.io/badge/runtime%20deps-0-0f6d63"></li>
<li><a href="https://github.com/meSingh/polinrider-cleaner/blob/main/LICENSE"><img alt="MIT licensed" src="https://img.shields.io/badge/license-MIT-64665f"></a></li>
</ul>

<p class="standfirst">
Detect and clean up after the <strong>PolinRider</strong> supply-chain campaign:
on a developer machine, on a personal GitHub account, or across a whole
organization. Written during a real incident across <strong>57 repositories and
304 branches</strong>, and still maintained against the campaign as it changes.
</p>

PolinRider is a DPRK-linked campaign tracked alongside the Contagious Interview
and Famous Chollima cluster. It has been running since December 2025 and, as of
April 2026, had reached **1,951 public GitHub repositories across 1,047 owners**.
It is not repository defacement. The repository changes are how it travels; what
it wants is your credentials.

## Why a tool rather than a checklist

The campaign backdates its commits. A script on the infected machine reads your
last commit's timestamp, winds the system clock back to it, amends that commit
with the payload, restores the clock, and force-pushes. One documented case
carried **267 days** of backdating.

The result is a commit with an ordinary date and your name on it. `git log`
shows you nothing. Detection has to be content scanning of every ref reconciled
against push events, and that is not something anyone does by eye across
fifty repositories.

## What it will not do

Worth knowing before you start, because a tool you run during an incident should
be honest about its limits.

- It **cannot prove you are clean.** A clean result means the current indicator
  set is absent. Signatures rotate. Rotate your credentials regardless.
- It **does not make a compromised machine trustworthy again.** The payload is a
  remote access trojan and an infostealer. Quarantining files is containment,
  not recovery; rebuilding from a clean OS install is recovery.
- It **changes nothing unless you tell it to.** Every destructive step is a dry
  run first, and `--apply` moves files to quarantine rather than deleting them.
- It is **an independent open source tool, with no warranty.** You are
  responsible for being authorised to run it against whatever you point it at.
  See [what this does not promise](./project/disclaimer.md).

## Why you can run this

You are being asked to run unknown code on a machine you may believe is
compromised. That deserves evidence rather than a promise.

| | |
|---|---|
| **Built during a real incident** | 57 repositories and 304 refs on the author's own account, two of ten restore points found infected, 267 days of backdating observed |
| **Indicators reviewed weekly** | Every review is [logged, including the weeks nothing changed](https://github.com/meSingh/polinrider-cleaner/tree/main/docs/indicator-reviews) |
| **Every commit GPG-signed** | The direct counter to the campaign's own backdated-amend technique |
| **Releases carry provenance** | A sigstore attestation proves the tarball came from this repository's CI |
| **Static analysis on every push** | CodeQL, Semgrep and OpenSSF Scorecard |
| **It scans itself** | CI fails if the repository matches its own indicators once its detection code is excluded |

Verification commands are in [verifying this repository](./reference/verifying.md).

## Where to go next

| Your situation | Read |
|---|---|
| Mid-incident, need to start now | [Quick start](./quick-start.md) |
| Want to understand the threat first | [What PolinRider is](./campaign/what-it-is.md) |
| Cleaning more than one thing | [Order matters](./guides/order.md) |
| Wondering why something is missing | [Why it works this way](./project/decisions.md) |
