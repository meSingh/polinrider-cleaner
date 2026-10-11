## Download

One program, `polinrider`, with its indicator set in `ioc/` beside it. Pick
the archive for your machine:

| Machine | Archive |
|---|---|
| macOS, Apple silicon | `polinrider-__TAG__-macos-arm64.tar.gz` |
| macOS, Intel | `polinrider-__TAG__-macos-x86_64.tar.gz` |
| Linux, x86_64 | `polinrider-__TAG__-linux-x86_64.tar.gz` |
| Linux, arm64 | `polinrider-__TAG__-linux-arm64.tar.gz` |
| Windows, x86_64 | `polinrider-__TAG__-windows-x86_64.tar.gz` |

Unpack it and run `polinrider` with no arguments. It asks what to check, one
question at a time, and changes nothing until you type `yes`.

## Verifying this release

Every archive here is checksummed and carries a build-provenance attestation.
Verify before you run any of it. This is a tool for recovering from tampered
code, so take it at its word about nothing.

```bash
# 1. the checksum matches
sha256sum -c SHA256SUMS --ignore-missing

# 2. the archive really was built by this repository's CI, at this tag
gh attestation verify polinrider-__TAG__-macos-arm64.tar.gz \
  --repo meSingh/polinrider-cleaner

# ...or offline, against the bundle attached here, with no network call
gh attestation verify polinrider-__TAG__-macos-arm64.tar.gz \
  --bundle polinrider-__TAG__.intoto.jsonl \
  --repo meSingh/polinrider-cleaner
```

The tag itself is GPG-signed:

```bash
git tag -v __TAG__
```

On macOS the binary is not notarised, so the first run is stopped by
Gatekeeper. After verifying it as above:

```bash
xattr -d com.apple.quarantine ./polinrider
```

## What is in it

One binary, no runtime dependencies. The GitHub checks call `git` and `gh`,
which you need only for those. Every change is shown as a dry run first and
made only on a typed `yes`.

1.x, the shell tool, is still available from the
[v1.0.9 release](https://github.com/meSingh/polinrider-cleaner/releases/tag/v1.0.9).

See the [README](https://github.com/meSingh/polinrider-cleaner#readme) to get
started, and [AGENTS.md](https://github.com/meSingh/polinrider-cleaner/blob/main/AGENTS.md)
if you are handing this to an AI agent.
