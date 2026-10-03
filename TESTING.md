# Testing the 2.0 beta

For the three-machine test before 2.0 is released.

**The beta is never run on the machine itself. Not installed, not on the PATH,
not a dry run, not `--version`.** This is a tool that moves files and rewrites
them, and it is a beta. It runs inside the sandbox container, where the home
directory is the container's own and nothing it does can reach the machine
underneath. That goes for the maintainer as much as for anybody else.

## 1. What each machine needs

Docker and git. Nothing else, and no Rust: the build happens in the container.

- **macOS:** Docker Desktop, running.
- **Linux:** the `docker.io` package or Docker Engine.
- **Windows:** Docker Desktop, and run the commands below from WSL or Git Bash.
  `polinrider-sandbox` is a bash script.

## 2. Start it

```bash
git clone -b v2 https://github.com/meSingh/polinrider-cleaner.git polinrider-v2
cd polinrider-v2
./polinrider-sandbox --beta
```

The first run builds the container image and takes a few minutes. After that
it is seconds. It builds the beta, installs it inside the container the way an
end user would have it, builds a sample and leaves you at a prompt.

## 3. Use it, as a user would

```bash
polinrider
```

That is the guided flow: four screens, one question on each, answered in
words. Type `computer`, which in here is the container, and press Enter to
check its whole home folder. (`folder` offers the code folders it finds, such
as `~/code`.) It checks, then shows a short summary of what it found and
asks one question.

The sample it finds:

| Where | What |
|---|---|
| `~/code/shop` | a project infected five ways |
| `~/code/blog` | a clean project |
| `~/.config/systemd/user/sysupdate-helper.service` | an infected login item |

None of it is live malware. Each file carries one indicator string from
`ioc/`, which is what the scanner matches.

Things worth trying at that question, and what should happen:

1. Press Enter, then type `sure`. It asks again each time and changes nothing.
2. Type `details`. It lists every finding with its path, and asks again.
3. Type `q`. It stops and says nothing further was changed.
4. Type `yes`. One file is stripped and two are moved, it checks again, and
   the last screen says what is left for you. `cat
   ~/code/shop/postcss.config.mjs` is three clean lines, and
   `ls ~/polinrider-quarantine-*` holds the originals with a `manifest.tsv`.
5. `./ci/beta.sh` puts the sample back, to go again.

Every guided run saves a full report in the home directory and names it on
the last screen.

Without the prompts:

```bash
polinrider --version              # which build, and how many indicators it has
polinrider check ~/code           # read-only check of the container and that folder
polinrider clean ~/code/shop      # what it would strip and move. --apply does it
echo $?                           # the exit code of the last command
```

## 4. What the result means

| Exit | Last lines | Means |
|---|---|---|
| `0` | `VERDICT: clean against the current indicator set.` | Nothing found |
| `1` | `VERDICT: no confirmed indicator.` and some `[review]` lines | Nothing confirmed. A human should read the `[review]` lines |
| `2` | A box saying `VERDICT: COMPROMISED` and one or more `[HIT]` lines | A confirmed indicator |
| `3` | An error on its own, no report | The scan could not run. Says nothing about infection |

On the sample, `polinrider check ~/code` exits 2. One `[review]` line is
expected in the container and is not a fault: `neither ss nor netstat
available, skipped`, because the image has no socket tool.

Every section should print at least one line. A section heading with nothing
under it is a bug worth reporting.

## 5. Your own code, read-only

To see what the beta makes of real projects without letting it near them, give
the sandbox a directory. It is mounted read-only at `/scan`: the beta can read
it and the kernel will not let it change a byte, whatever the beta does.

```bash
./polinrider-sandbox --beta ~/Sites
# then, inside:
polinrider check --fs-only /scan
```

`--apply` on `/scan` prints `QUARANTINE FAILED (Read-only file system)` and
changes nothing there. That is the point.

## 6. What this cannot test, and where that is tested instead

The container is Linux. **The macOS and Windows live checks, which read the
real machine's processes, sockets and login items, cannot run in it and are
never run on your machine.** They run on GitHub's own disposable machines: the
"Beta binaries" workflow builds the beta for Linux, macOS and Windows on every
push to `v2`, and on each one runs a check of that machine, cleans the sample
and verifies the original was kept. On macOS it also runs the unit tests and
the conformance corpus. The `[review]` lines a real macOS machine produces are
in that run's log.

The same workflow publishes each build as a download, for use inside any other
disposable container:

```bash
gh run download --repo meSingh/polinrider-cleaner --name polinrider-beta-linux-x86_64 --dir /tmp/prc-beta
tar -xzf /tmp/prc-beta/*.tar.gz -C /tmp && /tmp/polinrider-beta-linux-x86_64/polinrider --version
```

What that leaves untested until somebody chooses to run it on a real machine:
a real developer's Mac or Windows PC, with years of login items and editor
extensions, as opposed to a clean runner.

## 7. What to send back, for each machine

1. The operating system and version, and `uname -sm` where there is one.
2. Whether `./polinrider-sandbox --beta` got you to a prompt, and roughly how
   long the first run took.
3. What `polinrider --version` printed.
4. Step 3: whether Enter, `sure`, `details` and `q` left the file alone, and
   whether `yes` stripped one file and moved two. And how the screens read:
   anything that was hard to follow or easy to miss.
5. If you tried step 5: the exit code, roughly how long it took, and the
   `[HIT]` and `[review]` lines. They name your paths, so trim what you would
   not want in a thread.
6. Anything that looked wrong: a `[review]` line that is plainly noise, a
   `could not read` you did not expect, a section with nothing under it, a
   crash, or a wait long enough to wonder whether it had hung.

## Known before you start

- **Large files are hashed.** Every file between 10 MB and 300 MB under the
  directories given is read once to compare against known implant hashes. On a
  directory full of media or disk images that takes a while, and there is no
  progress line yet.
- **No GitHub.** This build does not scan or clean a remote. That is ported
  after this test.
- **"This machine cannot be trusted"** is printed under any `[HIT]`, including
  for a scan of one folder. The wording is carried over from 1.x and is wrong
  for a folder.
- **Windows through WSL or Git Bash has not been tried.** If
  `./polinrider-sandbox --beta` does not start there, that is a result.
