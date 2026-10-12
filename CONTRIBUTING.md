# Contributing

People run this during an incident, on machines they no longer trust, against
repositories they cannot afford to lose. That sets the bar.

## Rules

1. **Nothing is deleted.** Destructive actions are reversible: quarantine moves
   files, `clean` keeps the infected original, restore moves a branch pointer
   to a commit that still exists, and every GitHub fix keeps the old refs under
   `refs/polinrider/before-fix/`.
2. **Dry run by default.** Nothing changes without `--apply` or, in the guided
   flow, a typed `yes`.
3. **No runtime dependencies.** The binary is one Rust crate with no
   dependencies. It shells out to `git` and `gh` for GitHub and to the
   operating system's own tools for the machine check, and nothing else.
4. **A false `INFECTED` is worse than a missed `review`.** Someone acting on a
   false positive during an incident burns hours they do not have. If a
   legitimate project could contain the string, it belongs in `weak.txt`.
5. **The conformance corpus is the specification.** Add a case to
   `conformance/` for anything you add or fix, with a `why` that argues for
   the expected value.
6. **Nothing runs on your own machine.** The tool moves files and rewrites git
   history, and its tests exercise those paths. Everything runs in the sandbox
   container below or on a CI runner.

## Before you open a pull request

```bash
./polinrider-sandbox --all  # fmt, clippy, the Rust tests, the corpus and shell lint, in the container
```

`cargo fmt` can run on the host, since the sandbox mounts the repository
read-only. Compiling there is fine too. Running what was built is not.

The restore planner is the only code path that can destroy someone's work. It
lives in `src/remote_fix.rs`, and the conformance corpus builds two-wave attacks
offline and asserts that it refuses to restore onto an attacker commit. **Any
change to it needs a corpus case.**

`ci/scan-workspace.sh`, the vendorable CI scanner, is still shell. If you touch
it, run `./ci/selftest.sh` too and keep it working on bash 3.2, which is what
macOS ships: no `mapfile`, no associative arrays, no `${var,,}`, and guard
array expansion under `set -u` with `${arr[@]+"${arr[@]}"}`.

## Trying it in the sandbox

The sandbox is a container where the home directory is the container's own and
nothing the tool does can reach the machine underneath. It needs Docker and
git, and no Rust: the build happens in the container.

- **macOS:** Docker Desktop, running.
- **Linux:** the `docker.io` package or Docker Engine.
- **Windows:** Docker Desktop, with the commands below run from WSL or Git
  Bash. `polinrider-sandbox` is a bash script, and this path has not been
  tried yet.

```bash
git clone https://github.com/meSingh/polinrider-cleaner.git
cd polinrider-cleaner
./polinrider-sandbox --beta
```

The first run builds the image and takes a few minutes; after that it is
seconds. It builds the binary, installs it inside the container the way an end
user would have it, builds a sample and leaves you at a prompt.

### The machine and folder checks

```bash
polinrider
```

That is the guided flow: four screens, one question on each, answered in
words. Type `computer` (here, the container) to check the whole home folder, or
`folder` for the code folders it finds. The sample it finds:

| Where | What |
|---|---|
| `~/code/shop` | a project infected five ways |
| `~/code/blog` | a clean project |
| `~/.config/systemd/user/sysupdate-helper.service` | an infected login item |

None of it is live malware. Each file carries one indicator string from
`ioc/`, which is what the scanner matches.

At the question after the summary:

1. Press Enter, then type `sure`. It asks again each time and changes nothing.
2. `details` lists every finding with its path, and asks again.
3. `q` stops and says nothing further was changed.
4. `yes` strips one file and moves two, checks again, and the last screen says
   what is left for you. `cat ~/code/shop/postcss.config.mjs` is three clean
   lines, and `ls ~/polinrider-quarantine-*` holds the originals with a
   `manifest.tsv`.
5. `./ci/beta.sh` puts the sample back.

Every guided run saves a full report in the home directory and names it on the
last screen.

### The GitHub screens

On a pretend organization, with no network and no sign-in. The fixes push to
repositories that live inside the container:

```bash
polinrider guide --forge-state ~/demo-github
```

Type `organization`, then `acme`. It checks five repositories and finds the
payload in three:

| Repository | What happened to it | What can fix it |
|---|---|---|
| `acme/shop` | two branches attacked after they were clean, both on GitHub's push record | `restore`, or any other |
| `acme/website` | `main` pushed to, on record | `restore`, or any other |
| `acme/old-site` | infected for longer than GitHub remembers | `erase`, `remove` or `archive`. `restore` is not offered |

Then `details`, `each` (one repository at a time: a fix, its dry run, and a
push only on `yes`), `all` (one fix for all of them, which goes ahead only when
you type the owner's name) or `none`. `./ci/beta.sh` puts the organization
back. To see what a fix did:

```bash
git -C ~/demo-github/git/acme/shop.git log --oneline --all
git -C ~/demo-github/git/acme/old-site.git show main:README.md
cat ~/demo-github/changed/acme/old-site.description
```

Real GitHub has been tried against a throwaway organization. Try one of those
before a real one.

### Without the prompts

```bash
polinrider --version              # which build, and how many indicators it has
polinrider check ~/code           # read-only check of the container and that folder
polinrider clean ~/code/shop      # what it would strip and move. --apply does it
echo $?                           # 0 clean, 1 review, 2 compromised, 3 could not run
```

On the sample, `polinrider check ~/code` exits 2. One `[review]` line is
expected in the container and is not a fault: `neither ss nor netstat
available, skipped`, because the image has no socket tool. Every section
should print at least one line; a heading with nothing under it is a bug.

### Your own code, read-only

```bash
./polinrider-sandbox --beta ~/Sites
# then, inside:
polinrider check --fs-only /scan
```

The directory is mounted read-only at `/scan`: the tool can read it and the
kernel will not let it change a byte. `--apply` there prints
`QUARANTINE FAILED (Read-only file system)`, which is the point.

### What the sandbox cannot test

The container is Linux. The macOS and Windows live checks, which read the real
machine's processes, sockets, login items, registry Run keys and scheduled
tasks, run on GitHub's own disposable machines instead: the "Beta binaries"
workflow builds for Linux, macOS and Windows on every push to `main`, runs a
full check of each runner, cleans the sample and verifies the original was
kept. Its builds are downloadable for any other disposable container:

```bash
gh run download --repo meSingh/polinrider-cleaner --name polinrider-beta-linux-x86_64 --dir /tmp/prc-beta
```

What that leaves untested is a real developer's Mac or Windows PC, with years
of login items and editor extensions.

## Adding an indicator

See [`ioc/README.md`](ioc/README.md). Link a public source. Do not commit live
payload code, obfuscated or not. An indicator string, a hash, a package name or
a hostname is enough. The weekly review, and how to add an entry, is on the
[indicator reviews page](https://mesingh.github.io/polinrider-cleaner/indicator-reviews/).

## Conduct

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). The short version: people arrive
here mid-incident. Answer the question they asked.

## Signing

Commits and tags here are signed. If you are opening a pull request, signing
yours is appreciated but not required. The maintainer's signature on the merge
is what the release provenance chains to.

```bash
git config commit.gpgsign true
git config tag.gpgsign true
```

## Reporting

Use the issue templates. False positives and missed detections are the two most
useful reports. Include the exact command, the exact output and your operating
system. Output names your paths, so trim what you would not want public.
