# Handover

Everything a new session needs to pick this up. Written 2026-10-02, updated
2026-10-03 when the host-state checks landed.

Read [`HACKING.md`](./HACKING.md) first if you just want to run it. This file is
the state of the work and the reasoning behind it.

---

## Where things stand in one paragraph

`main` is **1.x**, the working shell tool, released at `v1.0.9` and still taking
weekly indicator updates from an automated routine. A branch called **`v2`** holds
a rewrite in progress: one Rust binary replacing seventeen shell scripts, a
documentation site, a container sandbox, and a conformance corpus that both
implementations answer to. **`v2` has never been pushed.** The Rust engine
passes every conformance case, for the filesystem checks and now for the
host-state checks too, and the local-repo cleaner and the guided flow are
built. On 2026-10-03 Mandeep chose to test the machine side on his three
machines now, with [`TESTING.md`](./TESTING.md) as the guide, and to have the
GitHub tracks ported to Rust after that test, not before it. **The test runs
inside the sandbox on each machine, never on the machine itself**: see the
first standing instruction below. `v2` is pushed to GitHub as a backup from 2026-10-03 and is **not** to be
merged to `main` until Mandeep has tested it on his three machines.

---

## Branch and remote state

| Ref | What it is | State |
|---|---|---|
| `origin/main` | 1.x, the released tool | `f58f2f7`, public. PRs #33 and #34 were squash-merged on 2026-10-03 |
| `main` (local) | Same as `origin/main` | Reset on 2026-10-03, see below |
| `v2` | The 2.0.0 work | Branched from `d418c54`. **Never pushed** |
| `scanner/one-walk` + tag `scanner-single-walk-1.x` | Backup of the 1.x scanner fix | Kept deliberately. Verified to merge into current `main` cleanly. **Do not merge it to `main`** — Mandeep wants it as a restore point only |
| `docs/social-preview` | Merged content, stale branch | Safe to delete |

Nothing in this work has been pushed. That is intentional: Mandeep reviews
before anything goes public.

### The `main` divergence: resolved

PRs #33 and #34 were squash-merged on GitHub on 2026-10-03 as `eb2b11a` and
`f58f2f7`, and local `main` was reset to `origin/main` the same day at
Mandeep's request. The two local commits it dropped, merge `c03419a` and
`98cd24f`, had the same tree as `eb2b11a`, so nothing was lost. They are in the
reflog if they are ever wanted.

---

## Running it

Full detail in [`HACKING.md`](./HACKING.md). The three commands:

```bash
./polinrider-sandbox --all     # lint, 9 self-tests, 80 conformance cases, clippy, 163 Rust tests
./polinrider-sandbox --demo    # build an infected sample and scan it with 1.x
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

**The corpus** — `conformance/`, 13 filesystem cases, 20 host cases, 7 clean
cases, 22 guide cases (14 of them GitHub, 10 of those the fixes), 16 refusal cases and 2 closed-pipe checks. A GitHub case must name any repository it expects to change; every other one must be exactly as it was afterwards. The filesystem cases are green
against both implementations. The host, clean and guide cases run against the
Rust engine only and print `skip` under the
shell, which cannot be handed a machine that does not exist. Running in CI and
in `--all`. Fixtures contain no
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

1. **Review the eight deliberate differences in ADR-0029.** Each is a judgement
   that the shell was mistaken. Three of them also apply to 1.x on `main`,
   where people are running it today: the Linux process name, the npm registry
   false positive and the `| shasum` false positive. Whether to fix those in
   the shell or let 2.0.0 carry them is Mandeep's call.
2. **What the host checks still do not have.** `LiveHost` on macOS has never
   been run, only built: the sandbox is Linux. The "Credential surface"
   inventory, the `stop it first` advice under an implant path and the review
   of extensions that reference campaign infrastructure are still shell only.
   Windows is refused without `--fs-only`.
3. **Delete the shell machine check** once 2 lands and the corpus is green on
   all three platforms. `--state` and `--resume` are not coming:
   [ADR-0030](./docs/adr/0030-2-0-does-not-checkpoint-a-scan.md).
4. **Reword "only moves files" wherever 1.x documentation says it**, before
   2.0 is released. `clean` edits source, and ADR-0031 says the claim has to be
   corrected and not left to mislead. The README on `v2` is still the 1.x one.
5. **The three-machine test, now.** [`TESTING.md`](./TESTING.md) is the guide:
   `./polinrider-sandbox --beta` on each machine, the guided flow on a sample,
   optionally real code mounted read-only, and what to send back. The results
   come back in the project thread. What a container cannot show, the macOS
   and Windows behaviour, comes from the "Beta binaries" workflow instead.
6. **The GitHub tracks in Rust, now.** Scanning an organization or an
   account, the push ledger, restore and remote cleaning are about 1,500 lines
   of shell in `lib/gh-*.sh`. On 2026-10-03 this was first put after the
   machine test; later the same day Mandeep tried the guided flow, missed the
   GitHub choices 1.x has, and asked for them back, which moves the port up.
   A mock of the screens was sent for his yes first (a first screen with
   `computer`, `folder`, `organization`, `account`, `everything`, then one
   repository at a time). Build in stages, and put a choice on the screen only
   when it really works: checking and reporting first, which changes nothing
   on GitHub; then `remove`; then `restore` and `erase`. It needs a boundary
   like `Host` so the corpus can drive it with local repositories, and real
   GitHub can only be tried from the sandbox with a network, a sign-in and a
   throwaway repository, all of which are Mandeep's to give.

   **Where it stands:** both stages are built and in the guided flow.
   `organization` and `account` are on the first screen. It checks that `gh`
   is installed and signed in and says what to run if not, lists the
   organizations, shows a progress screen, checks every branch and tag of
   every repository, and summarises: which repositories, how many branches,
   who pushed. Then `each`, `all`, `details` or `none`. One repository at a
   time offers `restore` (only where GitHub's push record gives a clean
   state), `erase`, `remove`, `archive` and `skip`; each shows a dry run and
   pushes only on a typed `yes`. `all` takes one fix for every repository
   and goes ahead only on the owner's name. The last screen is written from
   what was done. Mandeep approved the mock of all of it on 2026-10-03 ("The
   rest all looks great. Go ahead."), with one instruction for `archive`:
   the README notice goes at the very top, big and bold, and removes nothing
   that is already there.

   `./polinrider-sandbox --beta` builds a pretend organization with real
   attack history, and `polinrider guide --forge-state ~/demo-github` runs
   every screen and every fix against it with no network. `./ci/beta.sh`
   puts it back.

   **Not built:** `everything` (this computer, then GitHub); the progress
   screen for machine checks; the push-event sweep that narrows the list
   before copying every repository.

   **Nothing here has touched real GitHub.** Every test pushes to local bare
   repositories. The first real run is
   `./polinrider-sandbox --net --beta`, `gh auth login` inside it, and a
   throwaway repository, all of which are Mandeep's to give. Worth trying
   there first: a protected branch, a repository with a pull request open,
   and whether GitHub still serves the commit a branch was forced off.

   **Choices made while building the fixes that Mandeep has not ruled on**,
   all in ADR-0037: `restore` goes to the newest clean state on record, where
   1.x went to the earliest or to one before a time the operator gave;
   `remove` cuts the payload out of a build config and keeps the file, where
   1.x and the mock deleted it; `erase` takes its paths from the whole
   history, uses `git filter-branch` only, keeps empty commits and puts the
   clean config back as a commit; a push carries a lease so one that landed
   since the check is never overwritten; `none` and `fix` were added to the
   choices after the summary; the exit code stays 2 after a fix.

   **What Mandeep asked of the GitHub screens (2026-10-03), all agreed:** check that `gh` is installed and signed in
   first, and walk the operator through it if not; list their organizations
   to choose from; a progress screen while it works, the same one for every
   long job including machine checks; an "all at once" choice behind a warning
   and a second confirmation; `restore` only ever from GitHub's push record,
   never "the last commit", because this malware forges commit dates and
   scrambles history; `erase` from every commit as the alternative; and a new
   `archive` choice that marks an unused repository as infected and makes it
   read-only. The boundary already lists organizations and reports progress.

   **Two more differences from 1.x, for Mandeep's list** (with the eight in
   ADR-0029): a `tasks.json` that runs on folder open but carries no indicator
   is review in the remote check, as it already is in the machine check, where
   1.x's remote scan called it infected. And a path under `lib/` or `ci/` is
   no longer assumed to be the operator's own detection tooling: 1.x discounts
   every infected file in a directory with either name, in anybody's
   repository. On `v2` the shell's list had also grown `src/` and
   `conformance/`, added to quiet this repository's self-scan; that is a
   false negative in the v2 shell and is not carried into Rust.
7. **Release.** Only after 5 and 6, and only on Mandeep's word. 2.0.0 merges to
   `main` then. No backport of the ADR-0029 fixes to 1.x: 2.0 carries them.

`TASKS.md` in the repository root has the running list, including promotion work.
**It is git-ignored**, so it exists only on this machine; it carries outreach
notes that should not be public.

---

## External artifacts, and their state

| Thing | Where | State |
|---|---|---|
| 2.0 beta builds | "Beta binaries" workflow, `.github/workflows/beta-build.yml` | Builds Linux x86_64 and arm64, macOS arm64 and Windows x86_64 on every push to `v2`, and on each runner checks that machine, cleans a sample and verifies the original was kept. Workflow artifacts, not a release, 30-day expiry. The only place the binary runs on macOS and Windows, and the only place the macOS live checks run at all: a runner is disposable, a developer's machine is not |
| Homebrew tap | `meSingh/homebrew-tap` | Live. **Never install-tested** — `brew install mesingh/tap/polinrider-cleaner` still needs running once |
| Scoop bucket | `meSingh/scoop-bucket` | Live. Never tested on Windows |
| AUR package files | `~/polinrider-promotion/aur/` | Written, not published. Needs an aur.archlinux.org account |
| Weekly indicator routine | claude.ai routine `trig_01EGk2ww5vs9pqQpdYXaarz1` | **Running.** Mondays 08:00 IST, opens a draft PR when indicators change |
| GitHub Pages | Enabled, source "GitHub Actions" | Publishes from `main` only, so the new site is not live until 2.0.0 merges |

**awesome-incident-response PR #335 was closed**, not merged. The maintainer
said: *"lacks traction, plz reopen once you see more github stars for this one."*
That is a reopening condition, not a rejection of the tool. **awesome-security
#727 is still open** (that list has 328 open PRs and no pushes since January).

**Adoption is the real problem and has not moved.** Two stars. The highest-value
unstarted item is a pull request to the campaign's primary dossier,
`OpenSourceMalware/PolinRider` (91 stars), whose `## Remediation` section is six
manual steps and links to no tool at all. It has merged outside README pull
requests before.

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
- **Do not push without being asked.** Commit freely; pushing is his call. On
  2026-10-03 he asked for `v2` to be pushed as a backup. That covers `v2` only,
  and never a merge to `main` or a pull request.
- **Answer a question before acting on it.** Open questions get resolved first,
  not implemented speculatively.
- **All testing happens in the sandbox.** Not on his machine.
- **Commits are GPG-signed.** Do not disable signing.
- The scanner fix on `scanner/one-walk` stays a backup and does **not** go to
  `main`.

---

## Needing Mandeep

- Verify the Homebrew formula once: `brew install mesingh/tap/polinrider-cleaner`
- Review the eight differences from the shell in ADR-0029
- AUR account, if Arch packaging matters
- Whether `OpenSourceMalware/PolinRider` outreach happens, since it is the one
  thing likely to move adoption
