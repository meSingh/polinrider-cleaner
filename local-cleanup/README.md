# Local system cleanup

Check a workstation for PolinRider artifacts and quarantine what it finds. One
script per operating system, same checks, same output, same exit codes.

| Machine | Script | Needs |
|---|---|---|
| macOS | `check-macos.sh` | bash 3.2+, already installed |
| Linux | `check-linux.sh` | bash 4+, already installed |
| Windows | `check-windows.ps1` | Windows PowerShell 5.1, ships with Windows 10 and 11 |

**Every script is a dry run by default. Nothing is changed until you pass
`--apply` (`-Apply` on Windows), and even then nothing is deleted — confirmed
artifacts are moved into a quarantine directory with a manifest.**

---

## Run it

```bash
# macOS - dry run, list your real code directories
./check-macos.sh ~/Sites ~/Projects ~/work

# Linux
./check-linux.sh ~/src ~/code
```

```powershell
# Windows - run from the repository root
powershell -ExecutionPolicy Bypass -File .\local-cleanup\check-windows.ps1 -Roots C:\work,C:\src
```

The roots are the directories holding your git clones. The scan is only as good as
the roots you give it. With no roots the scripts use common defaults, which will
miss code kept elsewhere.

A first run takes 1–5 minutes, mostly spent reading IDE extension directories.

### Then, if you want it to act

```bash
./check-macos.sh --apply ~/Sites ~/Projects
```

```powershell
powershell -ExecutionPolicy Bypass -File .\local-cleanup\check-windows.ps1 -Roots C:\work -Apply
```

---

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Clean against the current indicator set |
| 1 | No confirmed indicator, but review items need a human |
| 2 | Confirmed indicator found |

Usable in a script: `./check-linux.sh ~/src || echo "needs attention"`.

---

## What it checks

| # | Area | Confirmed hit when |
|---|---|---|
| 1 | VS Code / Cursor / Windsurf / VSCodium extensions | an extension file contains an indicator string |
| 2 | `.vscode/tasks.json` | it runs on folder open **and** contains an indicator |
| 3 | Build configs (`postcss`, `tailwind`, `eslint`, `vite`, `next`, `rollup`, `webpack`, `babel`) | the file contains an indicator |
| 4 | `.woff` / `.woff2` files | the first four bytes are not `wOFF` or `wOF2` |
| 5 | `temp_auto_push.bat` | the file exists anywhere under your home directory |
| 6 | `package.json` and lockfiles | a known malicious package is referenced |
| 7 | Persistence — launch agents, systemd units, autostart, cron, Run keys, scheduled tasks, Startup folder | the entry contains an indicator |
| 8 | Shell and PowerShell startup files | an indicator, or a download piped into an interpreter |
| 9 | Git config and hooks | a hook contains an indicator |
| 10 | npm configuration | a non-default registry is configured |
| 11 | Running processes and live connections | an interpreter running inline code; a connection to known campaign infrastructure |
| 12 | Credential surface | reported for rotation, never a hit on its own |

Anything short of "confirmed" is reported as `[review]`: an editor task that runs
on folder open but has a plausible command, a config file with unexplained
trailing content, an active git hook, a non-empty crontab. Those are for you to
read, not for the script to judge.

---

## What `--apply` moves, and what it never touches

**Moved to quarantine:**

- IDE extensions containing an indicator
- `.vscode/tasks.json` that runs on folder open and contains an indicator
- `.woff` / `.woff2` files that are not fonts
- `temp_auto_push.bat`
- git hooks containing an indicator
- launch agents, systemd units, autostart entries, startup items and system cron
  files containing an indicator

**Never touched, reported instead:**

- **Build config files that contain an indicator.** The payload is appended to a
  real config file. Editing it in place leaves you guessing whether you got all of
  it. Delete the clone and re-clone after the remote is verified clean.
- **Shell and PowerShell startup files.** Removing the wrong line breaks your
  login shell. The script prints the file; you edit it.
- **Registry Run keys and scheduled tasks on Windows.** The script prints the
  exact `Remove-ItemProperty` or `Unregister-ScheduledTask` command; you run it.
- **Anything in the credential section.** Rotating is your job, from a clean machine.

Nothing is deleted, ever. The quarantine directory contains the moved files under
their original paths, a `manifest.tsv`, and `RESTORE.txt` with the command to put
them back.

```
~/polinrider-quarantine-20260824T101500Z/
├── manifest.tsv        original_path, quarantined_path, reason
├── RESTORE.txt
└── files/…             the moved files, original paths preserved
```

Keep it until the incident is closed. It is evidence.

A launch agent or systemd unit that has already been loaded stays running after
its file is moved. The script prints the `launchctl unload` or
`systemctl --user disable --now` command; run it, or reboot.

---

## After the scan

**Exit 2 — confirmed hit.** Disconnect from the network. Rotate every credential
in the report's credential section, from a different machine. Then decide whether
to rebuild, using the rule in
[root README, section 4](../README.md#should-the-machine-be-rebuilt): if any
persistence artifact was found, rebuild. Do not restore a backup taken after the
infection date. Delete every local clone.

**Exit 1 — review items only.** Read each one. A clean scan proves the current
indicator set is absent; signatures for this campaign rotate. Rotate your GitHub
tokens, SSH keys and cloud keys anyway.

**Exit 0 — clean.** Rotate anyway. Your tokens may have been taken from a
different machine or from a shared secret store.

In all three cases, apply the machine hardening from
[root README, section 5](../README.md#developer-machine). The single most
valuable line is `"task.allowAutomaticTasks": "off"` — it removes the
open-a-folder execution path outright.

---

## Two things the scripts deliberately do not do

**They do not read git history.** The propagation script backdates its commits, so
`git log` shows nothing wrong. Content scanning is the only reliable local signal.
To check a repository's history, use `../ci/scan-workspace.sh --all-refs`, and to
check the remote use [`../org-cleanup/`](../org-cleanup/) or
[`../personal-cleanup/`](../personal-cleanup/).

**They do not clean `node_modules`.** It is excluded from every scan. If a
lockfile references a malicious package, delete `node_modules`, remove the
dependency, and reinstall with `npm ci --ignore-scripts`.

---

## The report

Every run writes a full transcript to `~/polinrider-report-<timestamp>.txt`
(`-Report` / `--report` to change the path). Send that file, not a screenshot,
when you ask someone for help — and read it before you send it: it lists paths
from your machine.
