# 0003. Evidence lives in a temporary directory

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

Scanning an account mirror-clones every repository. Those mirrors hold live malware.

The first version wrote them to `./evidence`, inside the checkout. An editor indexes that, and a stray `git add -A` republishes the payload from the operator's own account.

Moving them to `~/.polinrider/evidence` fixed that and created a worse problem: a hidden directory under `$HOME` is one nobody looks at again, so infected mirrors sit there for months.

## Decision

Evidence defaults to `$TMPDIR/polinrider-evidence`, mode 700, which the machine clears on restart. `prc_prepare_out` refuses to write mirrors inside a git working tree at all. `--out` and `POLINRIDER_EVIDENCE_DIR` override it, and `--purge-evidence` removes it deliberately.

## Consequences

Forgetting about the evidence becomes the safe outcome rather than the dangerous one.

**The cost is real.** `restore.sh` reads pre-attack commits out of the mirror, so a restart can end the recovery window. When restore candidates exist, the tool says so and gives the command to copy those mirrors somewhere durable.

Re-running a scan after a restart re-clones everything, which on a large account is slow.

## Related

- [ADR-0011](./0011-pre-attack-commits-are-fetched-not-assumed-present.md), which is why the mirrors matter at all
