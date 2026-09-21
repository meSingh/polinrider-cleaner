# Exit codes

Every scanner in this repository uses the same four codes. They are a contract:
scripts and CI can depend on them.

| Code | Meaning |
|---|---|
| `0` | Clean against the current indicator set |
| `1` | Review items only; nothing confirmed |
| `2` | A confirmed indicator hit |
| `3` | **The scan could not run** |

## Why 3 exists

`2` used to mean both "confirmed infection" and "could not run", which meant
pointing the tool at a path that did not exist printed the full compromise
playbook. A tool that reports a compromise it did not find is worse than one
that reports nothing.

`3` is the separate answer for "I was unable to tell you", and it covers a
missing dependency, an unreadable path, an empty indicator set and an
unauthenticated GitHub CLI. See
[ADR-0002](../project/decisions.md).

> [!WARNING]
> A clean result is not proof.

## A clean result is not proof

`0` means the current indicator set is absent. Signatures rotate; an older or
rotated variant may have been here and left. Rotate your GitHub tokens, SSH keys
and cloud keys regardless of the exit code.
