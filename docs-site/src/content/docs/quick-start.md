---
title: Quick start
---

One program. Download it, verify it, run it. It asks what you need, one
question at a time, and tells you what it found before it offers to change
anything.

1. Download the archive for your machine from the
   [latest release](https://github.com/meSingh/polinrider-cleaner/releases/latest):
   macOS (Apple silicon or Intel), Linux (x86_64 or arm64) or Windows (x86_64).
2. Verify it:

   ```bash
   sha256sum -c SHA256SUMS --ignore-missing
   gh attestation verify polinrider-v2.1.0-macos-arm64.tar.gz --repo meSingh/polinrider-cleaner
   ```

3. Unpack it and run it with no arguments:

   ```bash
   tar -xzf polinrider-v2.1.0-macos-arm64.tar.gz
   cd polinrider-v2.1.0-macos-arm64
   ./polinrider
   ```

Keep the `ioc/` folder beside the binary. It is the indicator set, and the
binary refuses to scan without it. On macOS the binary is not notarised, so
Gatekeeper stops the first run; once you have verified it, run
`xattr -d com.apple.quarantine ./polinrider`.

Or install it with a package manager, which puts the binary and `ioc/` in
place:

```bash
brew install meSingh/tap/polinrider-cleaner    # macOS and Linux
scoop bucket add mesingh https://github.com/meSingh/scoop-bucket
scoop install polinrider-cleaner               # Windows
```

On Debian and Ubuntu, releases since 2.1.0 carry a `.deb` for x86_64 and arm64.
Verify it the same way, then `sudo apt install ./` followed by its file name.

:::note
It changes nothing until you type `yes`. Everything else on this site can wait
until it has told you what it found.
:::

## The first question

| Answer | What it checks |
|---|---|
| `computer` | This machine: your whole home folder, plus what starts by itself, shell startup files, git and npm settings, running programs and open connections |
| `folder` | One folder of code. It suggests the code folders it finds |
| `organization` | Every repository, branch and tag of a GitHub organization |
| `account` | The same for your own GitHub account |
| `everything` | This computer first, then GitHub |

Answers are words, never numbers. `q` leaves at any point.

## If it finds something

Do not start with the repositories. Cleaning a remote while an infected laptop
still holds a valid token puts you back where you started within minutes, and
that is documented behaviour of this campaign rather than bad luck.
[Order matters](/polinrider-cleaner/guides/order/) explains the sequence.
