# 0042. Python startup files and npm's own files are checked as persistence

| | |
|---|---|
| **Status** | Proposed |
| **Date** | 2026-10-13 |

## Context

Two persistence mechanisms were found by the weekly review and left for a
maintainer, and both survive the cleanup this repository documents.

**npm's own program.** The
[2026-09-14 review](https://mesingh.github.io/polinrider-cleaner/indicator-reviews/2026-09-14/)
(technique 1) found `npm/lib/cli.js`, a few hundred bytes and four lines as
npm ships it, replaced with a copy of about a megabyte with the payload after
a long run of whitespace. Every `npm`, `npx` or `npm exec` then starts the
payload again. Someone who cleans every repository, rotates every credential
and quarantines every implant path is reinfected by their next `npm i`. The
review suggested comparing the file against a size threshold, following
links. It has been carried as an open gap every week since.

**Python's startup files.** The
[2026-10-05 review](https://mesingh.github.io/polinrider-cleaner/indicator-reviews/2026-10-05/)
(technique 7) found the campaign installing Python and a stager, and named a
`.pth` file as how the `pybitjs` package re-runs on every interpreter start.
Python executes each line of a `.pth` file in a `site-packages` directory that
begins with `import`, every time any Python starts. Nothing here looked at
them.

## Decision

**npm's own program files.** In every npm installation in the usual places,
`lib/cli.js`, `lib/cli/entry.js`, `bin/npm-cli.js` and `bin/npx-cli.js` are
read through any link. The places: global installs for every user under the
host's system root (`/usr/lib`, `/usr/local/lib`, `/opt/homebrew/lib`,
Debian's `/usr/share/nodejs`, MacPorts, `n`, and `Program Files` on Windows),
the version managers under the home directory (nvm, Volta, fnm, asdf, mise,
nvm-windows), the common user prefixes, and any `prefix=` in `~/.npmrc`.

- An indicator in one is a confirmed hit, quarantined with `--apply`, with the
  advice to reinstall Node.js before running npm again. The file is moved like
  any other confirmed artifact: npm then does not run at all, which is better
  than npm running the payload.
- Without one, a file larger than 64 KB, a line longer than 4000 characters or
  code after a run of 64 or more blanks on one line is review: "looks altered".
- `~/.npmrc` is read for the two settings that make npm run code without a
  file of npm's changing: `onload-script`, and `node-options` with a flag that
  preloads a module. Review, or a hit with an indicator in the value.

**Python startup files.** Every `.pth` in a `site-packages` or `dist-packages`
directory under the scanned paths, in the `.venv` and `venv` environments the
walk lists without entering, in this user's Python under the home directory
(the user site, pyenv, uv, pipx, conda and others), and, with a host to read,
in Python installed for every user under the system root.

- An indicator anywhere in the file, a campaign package name included, is a
  confirmed hit, quarantined with `--apply`. Python skips a `.pth` that is not
  there, so moving it breaks nothing but the payload.
- A line Python executes that decodes, downloads, starts a process or opens a
  socket (`exec(`, `eval(`, `base64`, `subprocess`, `os.system`, `urllib` and
  the like), or that is longer than 2000 characters, is review.
- Everything packaging tools write is clean: path entries, `import _virtualenv`,
  setuptools' distutils shim, editable-install finders, namespace-package
  lines, easy-install's path arithmetic, pywin32's bootstrap and coverage's
  start-up hook. Two public ones that do use `exec`, pytest-cov's and
  python-hunter's, are recognised by what only they say.
- A `.pth` anywhere other than a site directory is not read. Nothing runs it,
  and PyTorch saves its models with the same extension.

Both checks are in `src/runtimes.rs`. Machine paths are read under the home
directory given and the host's system root, so the corpus supplies them as
files (ADR-0029). Neither runs Python or npm to ask where they are: running a
tool suspected of carrying the payload to answer a question its files answer
is not a trade worth making.

## Consequences

Better: the two pieces of persistence the reviews named as defeating the
documented cleanup are looked for, on the machine and in every project's
virtual environment, and a hit says what has to be reinstalled.

Worse:

- **Only the usual places.** npm installed under an unusual prefix that is not
  in `~/.npmrc`, or a Python built into `/opt/something`, is not found. The ok
  line says how many installations were checked, so "none found" is visible,
  but a missed installation is silent.
- **The size and shape rules are heuristics.** An attacker who keeps
  `lib/cli.js` small, with no padding and no indicator (a one-line `require`
  of a file elsewhere), passes. Only `lib/cli.js` and the three shims are
  read, not the rest of npm.
- **The `.pth` tells are a list.** A stager written to avoid every word on it,
  with no indicator, is clean. A legitimate package that `exec`s in a `.pth`
  and is not one of the two recognised is a review line on every machine that
  installs it. The recognised ones are matched by text an attacker could copy;
  an indicator still makes such a file a hit, but a lookalike without one is
  clean.
- **Quarantining npm leaves npm broken** until Node.js is reinstalled. That is
  the intent, and the finding and the guided flow both say so, but someone who
  runs `--apply` without reading will find npm gone.
- **More of the home directory is read.** Every site directory under each
  version manager is listed and each `.pth` in it read. These are small
  files, a few dozen on a typical machine.
- **`onload-script` and `node-options` are review when set.** Both have
  legitimate uses, so someone who uses either gets a review line on every run.
- **Platform paths are guesses for the platforms the sandbox cannot run.**
  The macOS and Windows locations are from each tool's documentation, not from
  a machine, and only CI's beta build runs on either.

## Related

- ADR-0029, the host boundary both checks read through.
- ADR-0025, the walk, which now lists Python environments without entering
  them.
- ADR-0041, why a campaign package name in a `.pth` counts only as a whole name.
