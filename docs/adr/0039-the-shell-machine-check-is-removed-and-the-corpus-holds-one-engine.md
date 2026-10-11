# 0039. The shell machine check is removed, and the corpus holds one engine

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-03 |

## Context

ADR-0026 said the shell would be replaced by one binary built against a
corpus, and that the replaced scripts would be deleted, not wrapped. The plan
the maintainer accepted put the machine check first: delete it once every
check it made was in Rust on Linux, macOS and Windows.

That became true on 2026-10-03. The last three shell-only checks were ported,
then Windows (ADR-0038), and the Windows probes ran on a real Windows machine
in CI and read real answers. The maintainer was asked whether to delete now
or wait, and chose the removal once Windows was covered.

## Decision

**Removed from `v2`:** `machine-cleanup/check-macos.sh`, `check-linux.sh`,
`check-windows.ps1`, their README, the library they shared
(`lib/local-common.sh`), and the two self-tests that existed only to test
them (`selftest-implant.sh`, `selftest-walk.sh`). About 1,800 lines.

**`polinrider.sh --machine` now says where the machine check went and exits
3.** It does not call the binary. A wrapper nobody tests is how two
implementations drift, and exit 3 is the honest code: this computer was not
checked. The options that belonged to the machine check (`--roots`, `--jobs`,
`--fs-only`, `--report`, `--state`, `--background`, `--resume`) are refused
by name and not ignored. `--path` is untouched: it never used the machine
check, it uses the repository scanner in `ci/`.

**The corpus runs one implementation.** `conformance/run.sh` no longer has a
shell adapter, runs the binary by default and fails if asked for the shell.
No case is skipped any more. The sandbox suite and CI run it once.

**The site's machine guide is the authoritative one**, rewritten for the
binary. The README under `machine-cleanup/` it used to defer to is gone.

**`./polinrider-sandbox --demo` is `--beta`.** The demo scanned a sample with
the shell check. `--beta` scans the same sample with the binary.

## Consequences

Better: one machine check, tested, on three platforms. The Windows check is
no longer a script nothing could exercise. Nobody has to keep three scripts
and a library in step with a fourth implementation by hand.

Worse:

- **The corpus has lost its second opinion.** The filesystem cases used to
  pass only when two independent implementations agreed. Now a case that
  pins a mistake is caught by nobody but a reader. The `why` on every case
  was always meant to be the argument; it is now the only one.
- **The differences from the shell in ADR-0029 are final by default.** The
  maintainer asked to review them and had not ruled when this was removed.
  The eight are still listed there with their reasoning, and each can be
  changed in Rust, but there is no longer a shell behaviour to compare
  against outside git history.
- **`main` still ships the shell machine check**, as 1.x. Somebody on `main`
  and somebody on `v2` now get different answers in the eight places
  ADR-0029 lists, three of which are bugs in 1.x.
- **`polinrider.sh` on this branch is half a tool.** It still drives the
  GitHub scripts and the folder scan, and refuses the machine. The binary's
  guided flow does all three. The rest of the shell goes when the GitHub
  fixes have run against real GitHub.
- **The user documentation is now further out of step.** The README on this
  branch is the 1.x one with the references corrected, not a 2.0 README.
- **Windows has been seen on one machine**, a CI runner.

It can be undone: the scripts are in git history at `55e56d5`.

## Related

- ADR-0026, one binary built against a corpus.
- ADR-0029, the differences from the shell.
- ADR-0038, Windows through the same boundary.
