# 0025. Walk the filesystem once, with -prune, and checkpoint it

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-16 |

## Context

A machine scan of a backup drive holding a few hundred gigabytes of old
projects ran for six hours and then died when the session logged out. Nothing
was saved, so the next attempt would have started from zero.

Two causes, both in `lib/local-common.sh`.

The first is a `find` idiom. Eight checks each ran their own walk over every
root, written as `find "$root" ... -not -path '*/node_modules/*'`. That predicate
filters what `find` prints. It does not stop the walk: `find` still descends into
every `node_modules` and tests every file inside it. Old project trees are
mostly `node_modules` by file count, so the scan visited millions of files it
would never report, eight times over. The correct form is
`-name node_modules -prune`, which stops at the directory.

The second is that nothing was checkpointed. A scan that takes hours has to
survive the terminal closing, the machine sleeping, and, when it cannot survive
something, has to pick up where it stopped.

The single-process structure was not the cause. Parallelism was considered and
rejected as the fix, because the work being parallelised was work that should
not have been done at all.

## Decision

The walk happens once per run, with `-prune` on `node_modules`, `.git`, caches,
trash and `Library`, and its output is written to a state directory
(`~/polinrider-scan-<timestamp>/`). Each check greps that list instead of
walking. `.git` directories are listed in a second, equally pruned walk so
hooks can still be read without entering object stores.

Every check runs through `run_check`, which records the check's name and the
running hit and review counts in the state directory when it completes.
`--resume` reuses the saved file list and skips recorded checks, and the
verdict counts what the interrupted run found.

`--background` re-executes the scan detached, under `caffeinate -i` on macOS so
the machine does not sleep, with output in the state directory's log.

The one per-file step that is genuinely heavy, hashing up to 200 large files
against the implant hash list, runs across cores with `xargs -P`.

`node_modules` is not walked at all. The campaign's malicious packages are
caught by name in manifests and lockfiles, and the payload it plants lives in
the project's own config files and `public/` fonts, never inside a dependency.

## Consequences

Better: a scan that took six hours takes minutes, and a scan that is
interrupted resumes instead of restarting.

Worse, and worth knowing:

- **A file that only exists inside a pruned directory is invisible.** A
  malicious package that has been installed is not content-scanned; it is
  caught by name from the lockfile. If the campaign starts shipping payloads
  that live only inside `node_modules` and are not named in any manifest, this
  check misses them. The prune list is one variable and easy to change.
- **The state directory lists every file path under the roots.** It is not
  infected content, but it is an inventory of the machine, and it lives under
  `$HOME`. It is in `.gitignore`. Delete it when the incident is closed.
- **`--background` survives the terminal closing and the machine sleeping. It
  does not survive a logout or a reboot.** Those kill the session's processes
  regardless of `nohup`. That is what `--resume` is for, and the message printed
  when it starts says so.
- **A resumed run re-reads the file list as it was.** Files added after the
  original walk are not seen. Run without `--resume` for a fresh walk.
- **Roots that overlap are walked twice.** They always were; this does not fix
  it, and the doubled counts are cosmetic.

## Related

- ADR-0003, evidence lives in a temporary directory: the state directory is
  deliberately not there, because a resume after a reboot needs it to exist.
- ADR-0023, parse before run: a scan that runs for an hour is exactly the case
  that record protects.
