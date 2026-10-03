# Testing the 2.0 beta on a real machine

For the three-machine test before 2.0 is released. This is the `v2` branch: a
beta, built and tested in a container so far, and never run on a real machine
until now. That is what this test is for.

**For the first pass, only dry runs on your real code.** Do not pass `--apply`
and do not type `yes` in the guided flow except on the sample in step 4. A dry
run reads and changes nothing.

## 1. Get the binary

**Without building.** Every push to `v2` builds the beta for Linux (x86_64 and
arm64), macOS (arm64) and Windows (x86_64). Each download is the binary with
its indicators beside it. With the GitHub CLI signed in:

```bash
gh run download --repo meSingh/polinrider-cleaner --name polinrider-beta-linux-x86_64 --dir /tmp/prc-beta
tar -xzf /tmp/prc-beta/*.tar.gz -C "$HOME"
"$HOME/polinrider-beta-linux-x86_64/polinrider" --version
```

Change the name for the machine: `polinrider-beta-linux-arm64`,
`polinrider-beta-macos-arm64` or `polinrider-beta-windows-x86_64`. Without the
CLI, the same files are under Artifacts on the latest "Beta binaries" run in
the repository's Actions tab. On Windows, unpack with `tar -xzf` in PowerShell
and run `polinrider.exe --version`.

`--version` prints the version, the commit it was built from and where it
found its indicators. If it says the indicators are not usable, stop and
report that: every scan would be refused.

To have a bare `polinrider` on the PATH, link the binary from a directory that
is on it, for example
`ln -s "$HOME/polinrider-beta-linux-x86_64/polinrider" ~/.local/bin/polinrider`.
The commands below are written for a checkout; with a download or an installed
copy, use that path or just `polinrider` in place of
`./target/release/polinrider`.

**Or build it.**

Needs `git` and `rustup`. Nothing else: the binary has no dependencies. The
repository pins the Rust version and `rustup` fetches it on the first build.

```bash
git clone -b v2 https://github.com/meSingh/polinrider-cleaner.git polinrider-v2
cd polinrider-v2
cargo build --release --locked
```

The binary is `target/release/polinrider` (`target\release\polinrider.exe` on
Windows). Run it from inside the checkout, so it finds the indicators in `ioc/`.

## 2. Check the machine, read only

macOS and Linux. Give it the directories your code is really in:

```bash
./target/release/polinrider check --report ~/polinrider-report.txt ~/Sites ~/Projects
echo "exit code: $?"
```

Windows. The live checks (processes, sockets, persistence) are not built for
Windows yet, so it is files only:

```powershell
.\target\release\polinrider.exe check --fs-only --report report.txt C:\path\to\code
"exit code: $LASTEXITCODE"
```

## 3. What the result means

| Exit | Last lines | Means |
|---|---|---|
| `0` | `VERDICT: clean against the current indicator set.` | Nothing found |
| `1` | `VERDICT: no confirmed indicator.` and some `[review]` lines | Nothing confirmed. A human should read the `[review]` lines |
| `2` | A box saying `VERDICT: COMPROMISED` and one or more `[HIT]` lines | A confirmed indicator |
| `3` | An error on its own, no report | The scan could not run. Says nothing about infection |

**Expect exit 1 on a machine you develop on.** An active git hook, a crontab
that is not empty, a global `core.hooksPath`, and `node -e` or `python -c`
processes started by an editor or a coding agent are all `[review]` lines and
all normal. Part of this test is finding out which of them are noise.

Every section should print at least one line. A section heading with nothing
under it is a bug worth reporting.

## 4. Watch it find and clean something, on a sample

This makes one small file holding one indicator string, in a temporary
directory. It is not malware. Delete the directory afterwards.

```bash
mkdir -p /tmp/prc-demo/proj
printf 'export default {}\n%280s%s\n' '' "$(grep -v '^#' ioc/strong.txt | grep -v '^$' | head -1)" > /tmp/prc-demo/proj/postcss.config.mjs

./target/release/polinrider clean /tmp/prc-demo           # dry run: exit 2, "would strip"
./target/release/polinrider clean --apply /tmp/prc-demo   # strips it, keeps the original
./target/release/polinrider clean /tmp/prc-demo           # exit 0
cat /tmp/prc-demo/proj/postcss.config.mjs                 # one line: export default {}
```

Then the guided flow on the same kind of sample. Recreate the file with the
`printf` line, run the binary with no arguments, choose `2`, type
`/tmp/prc-demo`, press Enter on an empty line, and answer the question. Try
pressing Enter at the yes or no prompt, and try `q`: neither should change the
file.

```bash
./target/release/polinrider
rm -rf /tmp/prc-demo
```

On Windows this step has not been tried at all. The same idea in PowerShell:

```powershell
$ind = (Get-Content ioc\strong.txt | Where-Object { $_ -and -not $_.StartsWith('#') })[0]
New-Item -ItemType Directory -Force $env:TEMP\prc-demo\proj | Out-Null
"export default {}`n" + (' ' * 280) + $ind | Set-Content $env:TEMP\prc-demo\proj\postcss.config.mjs
.\target\release\polinrider.exe clean $env:TEMP\prc-demo
```

## 5. Where quarantine lands

In your home directory, a new directory for every run that applies:
`~/polinrider-quarantine-<UTC date and time>/`. Inside it:

- `files/` holds each original at its full, resolved path. For the sample
  above that is `files/tmp/prc-demo/proj/postcss.config.mjs` on Linux and
  `files/private/tmp/prc-demo/proj/postcss.config.mjs` on macOS, where `/tmp`
  is a link
- `manifest.tsv` lists what was taken, from where and why
- `RESTORE.txt` says how to put a file back

A dry run prints the path it would use and creates nothing. `--quarantine DIR`
puts it somewhere else. Delete the sample's quarantine directory when you are
done; a quarantine from a real finding is evidence and is kept.

## 6. What to send back, for each machine

1. The operating system and version, and `uname -sm` where there is one.
2. Whether the build worked, and roughly how long it took.
3. The exit code of step 2 and roughly how long the check took.
4. The findings: `grep -E '\[(HIT|review)\]' ~/polinrider-report.txt`. The report
   names paths on your machine, so trim what you would not want in a thread.
5. Step 4: did the three `clean` runs give exit 2, then stripped, then exit 0,
   and did Enter and `q` leave the file alone in the guided flow.
6. Anything that looked wrong: a `[review]` line that is plainly noise, a
   `could not read` you did not expect, a section with nothing under it, a
   crash, or a wait long enough to wonder whether it had hung.

## Known before you start

- **macOS:** the live checks have been built on macOS and never run there. The
  first real run is yours.
- **Windows:** type-checks for Windows and has never been built or run on it.
  `--fs-only` only. A failure to build is a useful result.
- **Large files are hashed.** Every file between 10 MB and 300 MB under the
  directories you give is read once to compare against known implant hashes.
  On a directory full of media or disk images that takes a while, and there is
  no progress line yet.
- **No GitHub.** This build does not scan or clean a remote. That is ported
  after this test.
- **"This machine cannot be trusted"** is printed under any `[HIT]`, including
  for a scan of one folder. The wording is carried over from 1.x and is wrong
  for a folder.
