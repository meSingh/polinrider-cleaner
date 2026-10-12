# Handover

Everything a new session needs to pick this up. Written 2026-10-02, updated
2026-10-12 after the 2.0.0 release.

Read [`HACKING.md`](./HACKING.md) first if you just want to run it. This file is
the state of the work and the reasoning behind it. The sections from "The 2.0.0
decision" to "What is done" are the record of how 2.0 was built and still read
as history; the state sections above and below them are current.

---

## Where things stand in one paragraph

**polinrider 2.0.0 is released.** On 2026-10-11 the `v2` work was squash-merged
into `main` as #38 and tagged `v2.0.0`. The release workflow built, attested and
published five binaries (Linux x86_64 and arm64, macOS arm64 and x86_64,
Windows x86_64) with `SHA256SUMS`. `main` is now the Rust binary. 1.x is frozen
at `v1.0.9` and takes no backports, including the three 1.x bugs found during
the port. The Homebrew tap and Scoop bucket install the 2.0 binary but neither
has been install-tested. Nothing is in flight; what is left is below under
"What is next".

### How 2.0 got out

- **#36** (2026-10-09) brought main's 28 September and 5 October indicator
  reviews onto `v2`, taught the pattern engine `{n}`, `{n,}` and `{n,m}` (the
  5 October set otherwise stopped every scan with exit 3), made the folder and
  machine checks match `ioc/filenames.txt` as 1.x did, built five attested
  binaries in `release.yml`, and rewrote the README, `AGENTS.md` and the docs
  site for 2.0.
- **A real-GitHub test** against a throwaway organization passed except that
  `restore` was never offered. The cause: the events API lags an hour or more
  and no longer carries `payload.size`, so the push record came back empty and
  every push read as a force-push. Fixed in #36 by reading
  `/repos/{repo}/activity` instead (`src/remote.rs`), which shows a push within
  minutes and marks `force_push` itself. **Do not switch back.** The `restore`
  step was not re-run against real GitHub after the fix.
- **#37** removed the 1.x shell tool (`polinrider.sh`, `lib/`,
  `github-*-recovery/`, `ui/`, the shell self-tests). `ci/scan-workspace.sh`
  and the CI scan template stay shell: they scan every ref of history, which
  the binary does not do for a local checkout.
- **#38** squash-merged `v2` into `main`; `v2.0.0` was tagged and signed.

---

## Branch and remote state

| Ref | What it is |
|---|---|
| `main` | 2.0, the released binary. Protected: PR, signed commits, linear history, required checks |
| tag `v2.0.0` | The 2.0.0 release, on #38 |
| tag `v1.0.9` | The last 1.x release. Frozen |
| `v2` | Fully merged into `main`. Nothing new goes here |
| `ioc/YYYY-MM-DD` | One branch per weekly indicator review, kept after merge |
| `scanner/one-walk` + tag `scanner-single-walk-1.x` | Backup of the 1.x scanner fix. **Never merge it to `main`** |

`main` requires these checks: Shell lint and self-test, Scan this repository,
Static analysis, and the four platform builds (`linux-x86_64`, `linux-arm64`,
`macos-arm64`, `windows-x86_64`).

---

## Running it

Full detail in [`HACKING.md`](./HACKING.md). The three commands:

```bash
./polinrider-sandbox --all     # lint, the CI scanner self-test, clippy, the Rust tests, the conformance corpus
./polinrider-sandbox --beta    # 2.0 installed in the container, on the PATH, with a sample
./ci/docs-serve.sh             # the documentation site, with live reload
```

**Everything destructive runs in the container.** Mandeep asked for this
explicitly: the tool moves files and rewrites git history, and its tests
exercise those paths. `polinrider-sandbox` mounts the repository read-only,
gives the run its own `$HOME`, and has no network unless `--net` is passed.
Needs Docker Desktop running.

`--net` is required for anything that fetches. `--all` does not need it: the
crate has no dependencies, and the image now carries `rustfmt` and `clippy`
(see Gotchas).

---

## The 2.0.0 decision

[ADR-0026](./docs/adr/0026-2-0-0-is-one-binary-built-against-a-conformance-corpus.md)
is the full record. In short:

**Why a rewrite.** Seventeen separately runnable scripts and 2,933 lines of
documentation across 25 files, confusing enough that the author could not tell
which to run while cleaning a backup drive. And thirteen of the architecture
records exist because something broke — roughly eight of those failures are of a
class a type system removes.

**Why Rust and not Go.** One property: `Quarantine<DryRun>` has no method that
can move a file, so "quarantined during a dry run" fails to compile rather than
needing a reviewer to notice it. A `compile_fail` doctest proves it on every
build. See [`src/quarantine.rs`](./src/quarantine.rs).

**The corpus is the specification.** `conformance/` holds fixture trees as data
with their expected verdicts. The port is finished when it agrees with the shell
on every case, not when it compiles. Both implementations run the same suite.

**What this makes worse** is written into the ADR rather than left implied,
chiefly that "shell only, nothing to install" stops being true, and that is the
argument for running unknown code on a machine you believe is compromised.

---

## What is done

**The corpus** — `conformance/`, 13 filesystem cases, 27 host cases (5 of them a Windows machine), 7 clean
cases, 24 guide cases (15 of them GitHub, 10 of those the fixes), 16 refusal cases and 2 closed-pipe checks. A GitHub case must name any repository it expects to change; every other one must be exactly as it was afterwards. The filesystem cases were green
against both implementations until the shell machine check was removed
(ADR-0039); every case now runs against the one engine and none is skipped.
Running in CI and in `--all`. Fixtures contain no
payload: cases write `{{STRONG}}` and the runner substitutes from `ioc/` at build
time, so the repository stays clean and the corpus cannot drift from the
indicator set.

**The sandbox** — [ADR-0027](./docs/adr/0027-development-and-testing-happen-in-a-container.md).
`polinrider-sandbox` plus `.devcontainer/Dockerfile`. It found a real bug on its
first run (see Gotchas).

**The documentation site** — `docs-site/`, Astro Starlight with the Lucode theme,
17 pages, deploys to GitHub Pages from `main` only.
[ADR-0028](./docs/adr/0028-the-documentation-site-is-astro-starlight.md) records
that this reversed an earlier mdBook decision and what the Node dependency costs.

**The Rust engine** — 11 files in `src/`, no dependencies.

| Module | What it holds |
|---|---|
| `cli.rs` | Argument parsing and the preflight that refuses before anything is read |
| `verdict.rs` | Finding levels and the exit-code contract |
| `quarantine.rs` | The dry-run type-state |
| `walk.rs` | One pruned filesystem walk, shared by every check |
| `indicators.rs` | Loading `ioc/` |
| `checks.rs` | Implants, tasks.json, build configs, fonts, packages, git hooks, extensions, propagation |
| `scan.rs` | One scan: the walk, every check that applies, and rendering. In the library so a session can run more than one |
| `guide.rs` | The guided flow. Four screens, one question on each, answered in words. A summary first, the full list on `details`. Every prompt goes through one `Console` trait so a test can drive a session: ADR-0035 |
| `remote.rs` | GitHub through one boundary: `Forge` (`GitHub` through `gh` and `git`, or `Supplied` from a directory of bare repositories). Mirrors into an evidence directory and checks every branch and tag through git plumbing. Also the only four things that change GitHub: `push`, `set_description`, `archive` |
| `remote_fix.rs` | The four fixes, each as a plan and the function that does it: `restore`, `erase`, `remove`, `archive`. Nothing is checked out, every push carries a lease, and GitHub is asked afterwards what it shows. ADR-0037 |
| `guide_github.rs` | The GitHub screens that read: sign-in help, the organization list, the progress screen, the summary and the last screen. ADR-0036 |
| `guide_fix.rs` | The GitHub screens that change something: `each`, `all`, one repository at a time, the dry run, the yes, and what was done |
| `pattern.rs` | A small regular-expression matcher for `ioc/filenames.txt`, which is data written as patterns. A pattern it cannot honour stops the scan |
| `ui.rs` | The wordmark and the colours, and nothing else. Runs no command, reads no file. Colour never changes a character: ADR-0033 |
| `strip.rs` | Plans the cut for `clean`: what to keep of an infected build config, or why not to touch it. Pure, no I/O |
| `host.rs` | The boundary. `Host`, with `LiveHost` (asks the machine) and `Snapshot` (holds the answers as data). The only module that runs a command |
| `host_checks.rs` | Implant processes, persistence, shell startup files, global git config, npm config, resident interpreters, live connections |
| `sha256.rs` | Written out rather than depended on, proven against NIST vectors |

**Strict refusal.** Unknown flags, flags that exist but are not built, missing
roots, a root that is a file, a missing indicator set and an unusable quarantine
all exit 3 **before any file is read**. Three distinct messages, because they are
three different promises.

**The host-state checks**, recorded in
[ADR-0029](./docs/adr/0029-host-state-is-read-through-one-boundary-and-can-be-supplied.md).
Everything the scanner asks of the machine goes through one trait, and
`--host-state DIR` supplies the answers as files, which is what lets the corpus
assert on a process table. Without `--fs-only` no section prints `skipped` any
more. A probe that fails is a `[review]` line, never an `[ok]`.

Porting them found eight places where the shell was wrong, all listed in the
ADR with a case each. The one that matters most: **the implant process check
has never matched on Linux**, because the kernel cuts a process name to 15
bytes and the implant's is 17. Those eight are differences from 1.x that
Mandeep has not reviewed yet.

**The local-repo cleaner** —
[ADR-0031](./docs/adr/0031-clean-strips-an-appended-payload-in-place-and-never-touches-git.md).
`polinrider clean REPO...` strips a payload appended to a build config, in
place, keeping the infected original in quarantine. Dry run by default. It
cuts one shape and refuses the rest, and it never runs or touches git.
`check --apply` means what it always did.

**The guided flow** —
[ADR-0032](./docs/adr/0032-the-guided-flow-changes-something-only-on-a-typed-yes.md)
for the prompt rules and
[ADR-0035](./docs/adr/0035-the-guided-flow-is-four-calm-screens-answered-in-words.md)
for its shape. `polinrider` with no arguments. Four screens: what to check,
where, what was found, what to do now. Answers are words (`computer`,
`folder`, `yes`, `no`, `details`), never numbers. Step 3 is a summary in plain
words with no paths; `details` shows the full list. It changes something only
on a typed `yes`, only `q` leaves, and input that ends stops the session. A
report is saved on every run. Mandeep rejected the first version as too dense
for somebody under stress and approved a mock of this one before it was built:
**show him a mock before changing how it looks again.** `computer` checks
the whole home folder; `folder` suggests the code folders it finds.
`organization` and `account` check GitHub and fix it: item 6.

---

## What is next, in order

1. **Mandeep's verdicts on the open design choices.** Silence on an ADR means
   accepted; each can still be changed in Rust if he rules against one.
   - The eight deliberate differences from the shell in ADR-0029.
   - The GitHub fix choices in ADR-0037: `restore` goes to the newest clean
     state on record, where 1.x went to the earliest or to one before a time
     the operator gave; `remove` cuts the payload out of a build config and
     keeps the file, where 1.x deleted it; `erase` takes its paths from the
     whole history, uses `git filter-branch` only, keeps empty commits and
     puts the clean config back as a commit; a push carries a lease so one
     that landed since the check is never overwritten; `none` and `fix` were
     added to the choices after the summary; the exit code stays 2 after a
     fix.
   - Two more differences from 1.x: a `tasks.json` that runs on folder open
     but carries no indicator is review in the remote check, as in the
     machine check, where 1.x's remote scan called it infected; and a path
     under `lib/` or `ci/` is no longer assumed to be the operator's own
     detection tooling.
2. **Unicode-escaped loaders.** The 5 October review found that 18 of the 35
   confirmed infected repositories carry a loader written entirely in
   `\uXXXX` escapes, so none of the fixed strings in `strong.txt` appear in
   the raw bytes and every content check misses it. This is the highest-value
   engine change available. The likely shape is a normalisation pass that
   decodes escapes into a second buffer and matches both; a hit found only
   after decoding is reported but not stripped, since the strip works on byte
   offsets. The remote check would need to read blobs and match in Rust
   instead of `git grep -F`. Needs Mandeep's decision before it is built.
3. **Install-test the packages once each**, in a disposable VM or on a test
   machine: `brew install meSingh/tap/polinrider-cleaner` and
   `scoop install polinrider-cleaner`. Version bumps in both repositories are
   still manual on every release.
4. **Re-run `restore` against real GitHub** in a throwaway organization, now
   the push record comes from the activity API.
5. **Run 2.0 on a real Windows 11 machine.** Only CI runners have run it so
   far: no non-English locale, restricted execution policy or years of startup
   items yet.
6. **Engineering carried from 1.x**, none started:
   - the hash sweep is bounded to 10 to 300 MB (`src/checks.rs`), so the
     published small-file hashes cannot be carried;
   - package names are substring-matched rather than parsed from manifests;
   - no `.pth` / Python persistence check and no npm CLI persistence check;
   - the push-event sweep that narrows the GitHub list before mirroring;
   - `--purge-evidence` is not in the binary;
   - macOS binaries are not signed or notarised, so users run
     `xattr -d com.apple.quarantine`. Decide: sign, or keep documenting it;
   - the `account` and `organization` track guides are pointers and could
     move into the docs site.
7. **Packaging for the binary**: a winget manifest from the Windows zip, a
   `.deb`, and tap and bucket bumps from `release.yml`. AUR files are written
   but unpublished.

---

## External artifacts, and their state

| Thing | Where | State |
|---|---|---|
| 2.0.0 release | GitHub Releases, `v2.0.0` | Five binaries, `SHA256SUMS`, build attestations. Linux and Windows archives checked against the sums |
| Homebrew tap | `meSingh/homebrew-tap` | Ships the 2.0 binary as `polinrider`. **Never install-tested** |
| Scoop bucket | `meSingh/scoop-bucket` | Ships the 2.0 binary. Never tested on Windows |
| AUR package files | outside the repository | Written, not published. Needs an aur.archlinux.org account |
| Weekly indicator routine | claude.ai routine "PolinRider indicator watch (2.0)" | Mondays 08:00 IST. Opens a draft `ioc/YYYY-MM-DD` PR every week, change or no change. Replaced the 1.x routine on 2026-10-12 |
| Beta binaries workflow | `.github/workflows/beta-build.yml` | Builds and exercises every platform on each push to `main`. The only place the macOS and Windows live checks run |
| GitHub Pages | Source "GitHub Actions" | Publishes the docs site from `main` |

**awesome-incident-response PR #335 was closed**, not merged: the maintainer
asked for it to be reopened once the project has more stars. **awesome-security
#727 is still open.** Adoption is the real problem and has not moved.

---

## Gotchas, each of which cost real time

**Linux cuts a process name to 15 bytes.** `ps -o comm=` shows
`MicrosoftSystem`, not `MicrosoftSystem64`. Comparing whole names never
matches. Found by running a binary under the implant's name in the sandbox and
reading what `ps` printed; nothing else would have shown it.

**Joining an absolute path replaces what it is joined to.** The quarantine
destination was `root/files` joined with the source path minus a leading `/`.
On Windows `C:\code\x` has no leading `/`, so the destination was the source:
a move onto itself that reported success, and a strip that wrote the cleaned
file over the only copy of the original. A root given as `../code` escaped the
same way on any platform. Found by reading, the day before the first run on a
real machine, and it is the kind of thing the Linux sandbox cannot show.
Destinations are rebuilt from path components now.

**A quarantine under a scanned root was scanned.** The default used to be
`./polinrider-quarantine` in the working directory. Run from inside a project,
that put live malware in a git checkout, and the next scan walked it and
reported the machine still infected by its own evidence. The default is now a
new timestamped directory in the home directory, and the walk never enters a
quarantine.

**The sandbox image lacked `rustfmt` and `clippy`.** `rust-toolchain.toml` asks
for both and the slim image carries neither, so rustup tried to download them
on every cargo invocation and the whole Rust half of `--all` failed offline.
The Dockerfile installs them now. If the image predates 2026-10-03, rebuild it:
`./polinrider-sandbox --build`.

**The sandbox is read-only, so `cargo fmt` cannot run in it.** Format on the
host (it rewrites source and executes nothing), then check in the sandbox.
Compiling on the host with `cargo clippy` or `cargo build` is tolerated for
the same reason. Running what was built is not.

**`println!` panics on a closed pipe.** `polinrider --version | head -1`
crashed with a Rust panic, and that was the first thing the beta printed for
Mandeep. All output goes through `emit()` in `main.rs` now, `print_stdout` is
denied by clippy, and two corpus checks pin it. Do not pipe the binary through
`head` in a script and assume that is harmless.

**An installed binary could not find its indicators on macOS.** It looked for
`ioc/` beside the path it was started from, which through a link on the PATH
is the link. Fixed by resolving the path first. `./polinrider-sandbox --beta`
installs the binary through a link on purpose, so this layout stays tested.

**mdBook sets `html { font-size: 62.5% }`.** Irrelevant now the site is Astro,
but the lesson generalises: measure computed styles rather than eyeballing a
screenshot. The whole site rendered at 62% scale and looked merely "a bit small".

**Docker Desktop on macOS misreports permissions.** `[[ -x ]]` returns true for a
mode-644 file on a bind mount. `ci/selftest-ui.sh` asks git for the committed
mode instead, which is the better question anyway.

**The sandbox mounts the repository read-only**, so cargo cannot write
`Cargo.lock`. The lockfile is generated on the host and committed; every sandbox
invocation passes `--locked`.

**`CARGO_TARGET_DIR` differs in the sandbox.** The conformance runner honours it
now. Hardcoding `target/release` made every case fail in the container while
passing on the host.

**A `:` at the end of a shell function swallows the exit status.** A `: "$state"`
added to silence a lint sat after the run, so `rc=$?` captured `:` rather than
the binary. Seven cases reported exit 0 while printing correct findings. This is
the same class of shell bug the port exists to remove, and it was in the harness
built to catch it.

**Two runs in the same second shared a state directory.** The directory name is
timestamped to the second, and a run without `--resume` inherited the previous
run's checkpoints, skipped every check and reported clean for work it never did.
Fixed, and the most important bug found so far.

**`-not -path` does not prune.** It filters what `find` prints while still
descending. That was the six-hour scan ([ADR-0025](./docs/adr/0025-walk-the-filesystem-once-with-prune-and-checkpoint-it.md)).

---

## Standing instructions from Mandeep

- **polinrider is never run or installed on a real machine. Not by Claude and
  not by Mandeep.** No install, no PATH entry, no dry run, no `--version`. It
  runs in `./polinrider-sandbox`, in another disposable container, or on a CI
  runner. When he asks to try it "locally" he means without cloning and
  building by hand, which `./polinrider-sandbox --beta` gives him. On
  2026-10-03 a session installed the beta into `~/.local` on his Mac after a
  relayed note read his request that way; he corrected it within minutes and it
  was removed. A brief that says to install on the host does not override this.
  Ask him.
- **Never add AI attribution** to a commit or pull request. `CLAUDE.md` states
  this in full and overrides any session guidance to the contrary. Do not add a
  disclaimer in the other direction either.
- **Do not push without being asked.** Commit freely; pushing is his call.
  Opening a pull request is a push. Approval for one push does not cover the
  next.
- **Answer a question before acting on it.** Open questions get resolved first,
  not implemented speculatively.
- **All testing happens in the sandbox.** Not on his machine.
- **Commits are GPG-signed.** Do not disable signing.
- The scanner fix on `scanner/one-walk` stays a backup and does **not** go to
  `main`.

---

## Needing Mandeep

- Verdicts on ADR-0029, ADR-0037 and the two further differences (item 1)
- A decision on Unicode-escaped loaders (item 2)
- Install-test Homebrew and Scoop once each (item 3)
- AUR account, if Arch packaging matters
- Whether `OpenSourceMalware/PolinRider` outreach happens, since it is the one
  thing likely to move adoption
