---
title: PolinRider scanner false positives
description: Matches that look alarming and are harmless, each confirmed during a real cleanup, and what to do about them.
---

Each of these is harmless, and each was met during a real cleanup or while
testing the tool. Precision matters more than reach here: a false `INFECTED`
in a tool people run during an incident costs everyone real time.

| What you see | Why | What to do |
|---|---|---|
| Your own detection tooling reported: a scan workflow, its scripts, a copy of `ioc/` | It contains the indicator strings because it searches for them | Already handled for the usual paths, listed under each check below. Anything else of yours: in the CI scanner, add it with `--exclude` |
| `.vscode/settings.json` matched `folderOpen` | Only `.vscode/`**`tasks`**`.json` runs commands | The folder and computer checks skip `.vscode/` in their review-level scan. On GitHub it can appear as a review line, never as confirmed |
| A `.vscode/tasks.json` that runs on folder open, "verify the command by hand" | Teams use folder-open tasks for real work | Read the command. With a known indicator in it, it is reported as confirmed instead |
| Every `.woff2` in a repo flagged as "not a font" | Git LFS stores a text pointer instead of the font, and a zero-byte placeholder has no font bytes | Already handled: the folder, computer and GitHub checks skip LFS pointers and empty files |
| A config file flagged for "content after module end" | Flat configs legitimately open with `export default [` on line 1 and run long | A review line, never confirmed, and only when the remainder also looks like a payload |
| A README or incident write-up that names the campaign's indicators | Documenting the campaign means naming its indicators | Depends on the check; see below |
| Rendered documentation (HTML, a built docs site) reported | The documentation rules key off `.md`, `docs/` and README, which built output no longer matches | In the CI scanner, exclude your build output: `--exclude '(^\|/)site/'`. This repository's own self-scan excludes its built site, `docs-site/dist/` |

## Documentation, check by check

Each check treats documentation differently. This is what the code does
today.

- **The folder and computer checks** read indicators in particular kinds of
  file: `tasks.json`, build configs, fonts, git hooks and editor extensions.
  A `.md` file is none of these. Their wider, review-level scan of other files
  skips the paths treated as the user's own detection tooling: any `.md` file,
  anything under `docs/`, any README, `ioc/*.txt`, the tool's own scripts and
  workflows with `polinrider` in their name.
- **The GitHub check** never confirms a finding in those same paths, even
  when one contains an indicator string. It can still list such a file as a
  review line if it mentions a weak signal, such as `folderOpen`.
- **The CI scanner** (`ci/scan-workspace.sh`) skips `.md` files by default.
  `--scan-docs` includes them. It also always skips `.git/`, `node_modules/`,
  `.github/polinrider/`, `ioc/` and workflows with `polinrider` in their name,
  and `--exclude` adds more.

None of these exemptions covers `.mdx`, HTML or other formats. A page in one
of those that quotes a confirmed indicator can be reported as one.

Found a new one?
[Open an issue](https://github.com/meSingh/polinrider-scanner/issues/new?template=false-positive.md).
A missed detection is [a different template](https://github.com/meSingh/polinrider-scanner/issues/new?template=missed-detection.md),
and both are worth filing.
