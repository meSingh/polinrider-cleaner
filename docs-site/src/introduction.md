# polinrider-cleaner

Detect and clean up after the **PolinRider** supply-chain campaign: on a
developer machine, on a personal GitHub account, or across a whole organization.

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

## Where to go next

| Your situation | Read |
|---|---|
| Mid-incident, need to start now | [Quick start](./quick-start.md) |
| Want to understand the threat first | [What PolinRider is](./campaign/what-it-is.md) |
| Cleaning more than one thing | [Order matters](./guides/order.md) |
| Wondering why something is missing | [Why it works this way](./project/decisions.md) |
