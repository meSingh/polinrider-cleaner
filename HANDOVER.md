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
host-state checks too. What remains is checkpointing, the local-repo cleaner and
the guided flow.

---

## Branch and remote state

| Ref | What it is | State |
|---|---|---|
| `origin/main` | 1.x, the released tool | `f58f2f7`, public. PRs #33 and #34 were squash-merged on 2026-10-03 |
| `main` (local) | **Stale: 2 local commits, 2 behind** | Holds the old local merge of PR #33. Reset it to `origin/main`, see below |
| `v2` | The 2.0.0 work | Branched from `d418c54`. **Never pushed** |
| `scanner/one-walk` + tag `scanner-single-walk-1.x` | Backup of the 1.x scanner fix | Kept deliberately. Verified to merge into current `main` cleanly. **Do not merge it to `main`** — Mandeep wants it as a restore point only |
| `docs/social-preview` | Merged content, stale branch | Safe to delete |

Nothing in this work has been pushed. That is intentional: Mandeep reviews
before anything goes public.

### The `main` divergence: decided, one command left

PRs #33 and #34 were squash-merged on GitHub on 2026-10-03 as `eb2b11a` and
`f58f2f7`. Local `main` still holds the earlier local merge of #33: a merge
commit `c03419a` and `98cd24f`. Checked on 2026-10-03: `git diff main eb2b11a`
is empty, so those two commits carry nothing that is not already upstream, and
the working tree was clean.

What is left is `git checkout main && git reset --hard origin/main`. It was not
run by the session that verified it, because a hard reset wants Mandeep's own
word. The old commits stay in the reflog either way.

---

## Running it

Full detail in [`HACKING.md`](./HACKING.md). The three commands:

```bash
./polinrider-sandbox --all     # lint, 9 self-tests, 41 conformance cases, clippy, 68 Rust tests
./polinrider-sandbox --demo    # build an infected sample and scan it
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

**The corpus** — `conformance/`, 12 filesystem cases, 20 host cases and 9
refusal cases. The filesystem cases are green against both implementations.
The host cases run against the Rust engine only and print `skip` under the
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
3. **`--state` and `--resume` in Rust**, or a decision not to have them. They are
   currently refused with an explicit message, which is honest, but the shell has
   them and a backup-drive scan wants them.
4. **Delete the shell machine check** once 2 and 3 land and the corpus is green
   on all three platforms.
5. **The local-repo cleaner** — strip a payload from a working tree in place,
   without pull, reset or stash, preserving uncommitted work. Mandeep asked for
   this early on and it is still not built. It needs its own ADR because it
   changes what `--apply` means.
6. **The guided flow** — one CLI session walking triage, machine, credentials,
   remote, verify, prevent, without leaving the tool. This is the "fully
   automated flow" Mandeep described.
7. Then 2.0.0 merges to `main`.

`TASKS.md` in the repository root has the running list, including promotion work.
**It is git-ignored**, so it exists only on this machine; it carries outreach
notes that should not be public.

---

## External artifacts, and their state

| Thing | Where | State |
|---|---|---|
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

**The sandbox image lacked `rustfmt` and `clippy`.** `rust-toolchain.toml` asks
for both and the slim image carries neither, so rustup tried to download them
on every cargo invocation and the whole Rust half of `--all` failed offline.
The Dockerfile installs them now. If the image predates 2026-10-03, rebuild it:
`./polinrider-sandbox --build`.

**The sandbox is read-only, so `cargo fmt` cannot run in it.** Format on the
host (it rewrites source and executes nothing), then check in the sandbox.

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

- **Never add AI attribution** to a commit or pull request. `CLAUDE.md` states
  this in full and overrides any session guidance to the contrary. Do not add a
  disclaimer in the other direction either.
- **Do not push without being asked.** Commit freely; pushing is his call.
- **Answer a question before acting on it.** Open questions get resolved first,
  not implemented speculatively.
- **All testing happens in the sandbox.** Not on his machine.
- **Commits are GPG-signed.** Do not disable signing.
- The scanner fix on `scanner/one-walk` stays a backup and does **not** go to
  `main`.

---

## Needing Mandeep

- Verify the Homebrew formula once: `brew install mesingh/tap/polinrider-cleaner`
- Say the word on resetting local `main` (above): verified safe, not run
- Review the eight differences from the shell in ADR-0029
- AUR account, if Arch packaging matters
- Whether `OpenSourceMalware/PolinRider` outreach happens, since it is the one
  thing likely to move adoption
