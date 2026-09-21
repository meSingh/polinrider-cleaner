# Running this locally

Everything you need, in one page. Three commands do almost all of it.

| I want to… | Command |
|---|---|
| Check nothing is broken | `./ci/sandbox.sh --all` |
| Try the tool on a sample infected project | `./ci/sandbox.sh --demo` |
| Look at the documentation site | `./ci/docs-serve.sh` |

## First time only

You need **Docker Desktop** running. That is the only setup step.

```bash
brew install --cask docker    # if you do not have it
open -a Docker                # start it, wait for the whale in the menu bar
```

The first `./ci/sandbox.sh` builds a container image and takes a few minutes.
Every run after that is seconds.

## Why a container

This tool moves files into quarantine, rewrites git history and reads your home
directory. Testing it on your own machine means one wrong path away from moving
your real files. So the tests run inside a container instead, where:

- it sees its own home directory, never yours
- this repository is read-only to it, so it cannot damage the code
- it has no network unless we ask for one
- it is not root

Nothing it does in there can reach your Mac.

## Checking nothing is broken

```bash
./ci/sandbox.sh --all
```

Runs the linter, all nine self-tests, and the conformance corpus. Everything
should say `pass`. If something says `FAIL`, that is worth telling me about.

## Trying the tool for real

```bash
./ci/sandbox.sh --demo
```

This builds a fake workspace with two projects: `shop/`, infected five
different ways, and `blog/`, completely clean. Then it scans them and shows
you what it found, without changing anything.

It leaves you at a prompt inside the container so you can keep experimenting.
It prints the commands worth trying, including the one that actually moves the
bad files into quarantine so you can see what that does. Type `exit` when done.

The sample is not real malware. It contains one indicator string, copied from
`ioc/` when the demo ran, which is the thing the scanner matches on.

### What the exit codes mean

| Code | Meaning |
|---|---|
| `0` | Clean, as far as the current indicator set can tell |
| `1` | Nothing confirmed, but something needs a human to look |
| `2` | A confirmed indicator. This is a real finding |
| `3` | The scan could not run at all |

`3` matters: a tool that reports "clean" when it actually failed is worse than
one that says nothing.

## Looking at the documentation site

```bash
./ci/docs-serve.sh
```

Opens <http://localhost:3000>. Edit any file under `docs-site/` and the page
reloads by itself, so you can adjust the design by looking at it.

- `docs-site/src/**/*.md` — the words
- `docs-site/theme/custom.css` — colours, type, spacing, callouts
- `docs-site/src/SUMMARY.md` — the sidebar and page order

This one runs on your Mac rather than in the container, because rendering
Markdown is not dangerous and a read-only container would fight the
edit-and-see-it loop.

> [!NOTE]
> One thing that bites in `custom.css`: mdBook sets the root font size to
> 62.5%, so `1rem` is **10px**, not 16px. Every length is written as pixels
> divided by ten. `1.65rem` is 16.5px.

## Where things are

| | |
|---|---|
| `polinrider.sh` | The one command a user runs |
| `machine-cleanup/` | Scans one computer |
| `github-*-recovery/` | Scans and repairs GitHub accounts and organizations |
| `ioc/` | The indicator set. Plain text, read at runtime |
| `conformance/` | The specification: what a scan must return, as data |
| `docs-site/` | The documentation site |
| `docs/adr/` | Why things are the way they are, each with its cost |

## A limit worth knowing

The container is Linux. Your Mac is not. It catches most things, but
macOS-specific behaviour is only really tested by CI's macOS runner and by
running on your actual machine. A green sandbox is good news, not proof.
