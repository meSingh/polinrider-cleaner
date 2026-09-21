# Quick start

Two commands. It asks what you need, works out which scanner to run for the
machine you are on, and tells you what to do next.

```bash
git clone https://github.com/meSingh/polinrider-cleaner.git && cd polinrider-cleaner
./polinrider.sh
```

**This is read-only. It changes nothing.** Everything else on this site can
wait until it has told you what it found.

## Installing instead of cloning

```bash
brew install meSingh/tap/polinrider-cleaner   # macOS and Linux
polinrider
```

```powershell
scoop bucket add mesingh https://github.com/meSingh/scoop-bucket   # Windows
scoop install polinrider-cleaner
polinrider-check -Roots C:\work
```

## A large disk

A drive full of old projects takes a while. Two flags for that:

```bash
./polinrider.sh --machine --background    # detached; the terminal can close, the machine will not sleep
./polinrider.sh --machine --resume        # an interrupted run picks up where it stopped
```

`--background` survives the terminal closing and idle sleep. It does not survive
a logout or a reboot; that is what `--resume` is for, and it reuses the file list
and skips the checks that already finished.

## If it finds something

Do not start with the repositories. Cleaning a remote while an infected laptop
still holds a valid token puts you back where you started within minutes, and
that is documented behaviour of this campaign rather than bad luck.
[Order matters](./guides/order.md) explains the sequence.
