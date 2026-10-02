# Handover

Everything a new session needs to pick this up. Written 2026-10-02.

Read [`HACKING.md`](./HACKING.md) first if you just want to run it. This file is
the state of the work and the reasoning behind it.

---

## Where things stand in one paragraph

`main` is **1.x**, the working shell tool, released at `v1.0.9` and still taking
weekly indicator updates from an automated routine. A branch called **`v2`** holds
a rewrite in progress: one Rust binary replacing seventeen shell scripts, a
documentation site, a container sandbox, and a conformance corpus that both
implementations answer to. **`v2` has 16 commits and has never been pushed.** The
Rust engine passes every conformance case for the filesystem checks; the
live-host checks are the remaining work.

---

## Branch and remote state

| Ref | What it is | State |
|---|---|---|
| `origin/main` | 1.x, the released tool | `d418c54`, public |
| `main` (local) | **2 commits ahead of origin** | Holds a local merge of PR #33, which is still open on GitHub. Resolve before pushing |
| `v2` | The 2.0.0 work | 16 commits ahead of `origin/main`. **Never pushed** |
| `scanner/one-walk` + tag `scanner-single-walk-1.x` | Backup of the 1.x scanner fix | Kept deliberately. Verified to merge into current `main` cleanly. **Do not merge it to `main`** — Mandeep wants it as a restore point only |
| `docs/social-preview` | Merged content, stale branch | Safe to delete |

Nothing in this work has been pushed. That is intentional: Mandeep reviews
before anything goes public.

### The `main` divergence, to resolve first

Local `main` has a merge of the 21 September indicator review. The same work is
still open as PR #33 on GitHub, and PR #34 (28 September, 11 indicators) has not
been taken locally at all. Either merge both PRs on GitHub and reset local `main`
to match, or push the local merge and close #33. Do not leave it as it is.

---

## Running it

Full detail in [`HACKING.md`](./HACKING.md). The three commands:

```bash
./polinrider-sandbox --all     # lint, 9 self-tests, 18 conformance cases, clippy, 29 Rust tests
./polinrider-sandbox --demo    # build an infected sample and scan it
./ci/docs-serve.sh             # the documentation site, with live reload
```

**Everything destructive runs in the container.** Mandeep asked for this
explicitly: the tool moves files and rewrites git history, and its tests
exercise those paths. `polinrider-sandbox` mounts the repository read-only,
gives the run its own `$HOME`, and has no network unless `--net` is passed.
Needs Docker Desktop running.

`--net` is required for anything that fetches: `./polinrider-sandbox --net --all`
when cargo needs the registry.

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

**The corpus** — `conformance/`, 12 behaviour cases plus 6 refusal cases, green
against both implementations, running in CI and in `--all`. Fixtures contain no
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

**The Rust engine** — 2,291 lines across 9 files in `src/`, no dependencies.

| Module | What it holds |
|---|---|
| `cli.rs` | Argument parsing and the preflight that refuses before anything is read |
| `verdict.rs` | Finding levels and the exit-code contract |
| `quarantine.rs` | The dry-run type-state |
| `walk.rs` | One pruned filesystem walk, shared by every check |
| `indicators.rs` | Loading `ioc/` |
| `checks.rs` | Implants, tasks.json, build configs, fonts, packages, git hooks, extensions, propagation |
| `sha256.rs` | Written out rather than depended on, proven against NIST vectors |

**Strict refusal.** Unknown flags, flags that exist but are not built, missing
roots, a root that is a file, a missing indicator set and an unusable quarantine
all exit 3 **before any file is read**. Three distinct messages, because they are
three different promises.

---

## What is next, in order

1. **Host-state checks in Rust** — processes, sockets, npm config, crontab,
   persistence, shell startup files. These need an **injectable boundary** so a
   test can supply fake system state. That decision was made and recorded but not
   yet built; it is why `--fs-only` exists and why seven sections currently print
   `skipped`. This is the untested third of the scanner, and an untested check
   that silently stops matching is the failure Mandeep cares most about.
2. **`--state` and `--resume` in Rust**, or a decision not to have them. They are
   currently refused with an explicit message, which is honest, but the shell has
   them and a backup-drive scan wants them.
3. **Delete the shell machine check** once 1 and 2 land and the corpus is green
   on all three platforms.
4. **The local-repo cleaner** — strip a payload from a working tree in place,
   without pull, reset or stash, preserving uncommitted work. Mandeep asked for
   this early on and it is still not built. It needs its own ADR because it
   changes what `--apply` means.
5. **The guided flow** — one CLI session walking triage, machine, credentials,
   remote, verify, prevent, without leaving the tool. This is the "fully
   automated flow" Mandeep described.
6. Then 2.0.0 merges to `main`.

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
- Decide the `main` divergence above
- AUR account, if Arch packaging matters
- Whether `OpenSourceMalware/PolinRider` outreach happens, since it is the one
  thing likely to move adoption
