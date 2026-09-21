---
title: A machine
---

> **Being migrated.** The full guide currently lives in
> [`machine-cleanup/README.md`](https://github.com/meSingh/polinrider-cleaner/blob/main/machine-cleanup/README.md) and is authoritative until this page
> replaces it. Tracked in [ADR-0026](/project/decisions/).

**Read-only. Changes nothing.** Run this on every machine that has touched the
affected repositories, before you touch GitHub at all.

```bash
./polinrider.sh --machine --roots "$HOME/Sites $HOME/Projects"
```

It detects the operating system and runs the right check.

## What it looks at

The second-stage implant first, then IDE extensions, editor tasks that run on
folder open, build configs, fake fonts, the propagation script, known-bad
packages, persistence entries, shell startup files, git hooks, npm
configuration, live connections and your credential surface.

## What `--apply` does

Quarantines confirmed artifacts into a timestamped directory with a manifest and
restore instructions. **Nothing is ever deleted.** Build config files and shell
startup files are never touched automatically: the payload is appended to real
files, so the script reports them and you re-clone.

## Large disks

`node_modules`, `.git`, caches and trash are never walked. Malicious packages
are caught by name from manifests and lockfiles, and the payload lives in the
project's own files rather than inside a dependency. See
[ADR-0025](/project/decisions/) for what that misses.

`--background` detaches the run and keeps the machine awake; `--resume` picks up
an interrupted one. Neither survives a logout, which is what `--resume` is for.
