---
title: A machine
---

> **2.0 beta.** On the `v2` branch the machine check is the `polinrider`
> binary. The three per-system scripts under `machine-cleanup/` are gone:
> macOS, Linux and Windows are checked by the same code, through one tested
> boundary. See [the decisions](/project/decisions/), ADR-0026, ADR-0029 and
> ADR-0038.

**Read-only unless you say otherwise.** Check every machine that has touched
the affected repositories, before you touch GitHub at all.

```bash
polinrider
```

That is the guided flow: it asks what to check, one question at a time, and
changes nothing unless you type `yes`. Choose `computer`.

Without the questions:

```bash
polinrider check ~/code
```

A check of this computer and of that folder. It moves nothing. Add `--apply`
to move confirmed artifacts into quarantine. `--fs-only` checks the folder
alone and reads nothing of the machine, which is what you want for a backup
drive or a mounted image.

## What it looks at

The second-stage implant first: its files, its process, and on Windows its
scheduled task and Run entry. Then editor extensions, editor tasks that run on
folder open, build configs, fake fonts, the script the payload spreads with,
known-bad packages, what starts by itself, shell startup files and PowerShell
profiles, git hooks, npm configuration, interpreters running inline code and
live connections. Last, an inventory of the keys and credential files that
would have to be changed if anything else is confirmed. They are counted and
named, and never opened.

"What starts by itself" depends on the system:

| System | Read |
|---|---|
| Linux | systemd units, autostart entries, the user crontab, system cron |
| macOS | LaunchAgents and LaunchDaemons, the user crontab |
| Windows | registry Run and RunOnce values, both Startup folders, scheduled tasks |

## What changes, and when

Nothing is ever deleted.

- `polinrider check --apply` moves confirmed artifacts into a timestamped
  quarantine directory with a manifest and instructions for putting them back.
  It never edits a file.
- `polinrider clean DIR --apply` does that and one thing more: where the
  payload was appended to a build config, it cuts the payload out in place and
  keeps the infected original in quarantine. Your own uncommitted work in that
  file survives. It does not touch git.
- A crontab line, a shell startup file, a PowerShell profile, a Run entry and
  a scheduled task are never changed for you. The finding carries the command
  that removes it.

## Large disks

`node_modules`, `.git`, caches and trash are never walked. Malicious packages
are caught by name from manifests and lockfiles, and the payload lives in the
project's own files rather than inside a dependency. See
[ADR-0025](/project/decisions/) for what that misses.

A long check shows how far it has come. 2.0 does not checkpoint a scan: there
is no `--resume`, and ADR-0030 says why.

## Exit codes

`0` clean against today's indicators, `1` something needs a human look, `2` a
confirmed indicator, `3` the check could not run. `3` says nothing about
whether you are infected.
