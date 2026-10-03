# 0038. Windows is read through the same boundary, with PowerShell as the tool

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-03 |

## Context

The Rust engine refused Windows unless told `--fs-only`. The Windows machine
check was `machine-cleanup/check-windows.ps1`: 532 lines of PowerShell that
nothing tested, kept in step with the shell by hand. It is the last thing
standing between the Rust engine and deleting the shell machine check, which
ADR-0026 set as the point of the rewrite.

Windows differs from Linux and macOS in one way that matters here. What
starts by itself is mostly not a file in a folder. It is a value under a
registry Run key or a scheduled task, and neither can be read with `fs`.

## Decision

**`Platform` gains `Windows`, and `Host` gains two questions**: `run_keys`
and `scheduled_tasks`, each answering with a list of place, name and what it
runs. They are asked only on Windows. `Snapshot` takes them as two more
files, `run-keys` and `scheduled-tasks`, so a Windows machine is handed to
the corpus as data exactly as a Linux one is, and the cases run on Linux.

**`LiveHost` on Windows asks PowerShell.** Windows PowerShell 5 is part of
every supported Windows, so nothing is installed. Processes come from
`Win32_Process`, connections from `Get-NetTCPConnection`, tasks from
`Get-ScheduledTask`, Run values from `Get-ItemProperty`. Each script is a
constant in `host.rs`, uses no double quote and takes nothing from outside,
and joins its fields with a tab made inside the script, so no value has to
survive two layers of quoting. `reg.exe` and `schtasks` were considered and
set aside: their output is localised, and a parser written against English
headings stops matching on a German machine without anyone finding out.

**The checks are the PowerShell script's, with four changes:**

- The implant is matched by image name without regard to case and with or
  without `.exe`, and in a command line only as a whole name. The script
  matched `*name*`, which also matches a longer name that begins the same.
- A task named for the implant is confirmed wherever it sits. The script
  looked for one fixed task name. Tasks under `\Microsoft\` are otherwise
  checked for indicators and not listed for review, as in the script.
- Run values are filtered by the five names PowerShell adds, not by `PS*`.
  The script skipped any value whose name began with PS, which is a place
  to hide.
- `desktop.ini` in a Startup folder is not an item. A PowerShell profile
  line that is a comment runs nothing and is not a finding.

**A Run entry or a task is a new kind of finding, `Autostart`.** It cannot
be moved into quarantine, so the finding carries the PowerShell command that
removes it, with the names quoted for PowerShell, and the last screen of the
guided flow says to run those.

**Implant paths written with `%LOCALAPPDATA%` are read under the home
directory given**, on every platform. A Windows profile on a backup drive
checked from a Mac is now checked for them too.

## Consequences

Better: one engine on three platforms, and the Windows checks are tested on
every push by cases that run anywhere.

Worse:

- **The part that talks to Windows has run only on a CI runner.** The
  "Beta binaries" workflow now checks the Windows runner itself and fails if
  a probe does not answer. A runner is a clean English-language Windows
  Server. A developer's Windows 11 with a decade of startup items, a
  non-English locale, a restricted execution policy or constrained language
  mode has not been seen.
- **Four PowerShell processes per check**, each taking a second or more to
  start. The shell script paid this once.
- **The all-users PowerShell profile under the Windows directory is not
  read.** The script read it through `$PROFILE`. The per-user ones are read.
- **A scheduled task whose action is not a program**, a COM handler, has an
  empty command and is seen only by its name.
- **The unit tests still assume Unix paths** and are not run on Windows.
- `check-windows.ps1` was not deleted by this change. It went the same day
  with the rest of the shell machine check, on the maintainer's word, after
  the Windows probes had run on the CI runner: ADR-0039.

## Related

- ADR-0029, host state through one boundary: this is that boundary, extended.
- ADR-0026, one binary built against a corpus.
