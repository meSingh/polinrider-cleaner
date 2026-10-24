# 0046. Let `--purge-evidence` delete polinrider's own copies, and nothing else

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-25 |

## Context

The GitHub checks copy every repository into an evidence folder before they
change anything (ADR-0003, ADR-0036). Those copies hold live malware, and
they hold the commits from before the attack, which a restore may still
need. 1.x had `--purge-evidence` to delete the folder once the cleanup was
over. 2.0 left it out.

The default folder is under the system's temporary directory and is cleared
on restart, so forgetting it is the safe outcome there. With `--evidence DIR`
it is not: the copies stay until someone removes them by hand, and `rm -rf`
on a folder full of malware, typed from memory, is exactly the kind of step
this tool exists to take off a stressed person.

The repository's first non-negotiable rule is that nothing is ever deleted.
That rule protects the operator's files and their history. These copies are
neither: polinrider made them.

## Decision

`polinrider --purge-evidence [--evidence DIR]` reads the folder, shows what
it holds (how many copies, how large) and the reason to keep it, and asks
one question. Only the whole word `yes` deletes. `no` or `q` leave it, and
input that ends deletes nothing and exits 3.

It deletes only a folder that is polinrider's own, read twice, once before
the question and again after the answer:

- every entry is a copy (a `*.git` folder with a `HEAD`), the indicator
  lists a check writes before its first copy, or the `rewrite` folder a
  stopped erase leaves;
- the indicator lists are there, so a folder of somebody's own bare
  repositories is not taken for one;
- the folder is not a link, and nothing inside is followed through a link.

Anything else, and the whole folder is refused with exit 3, naming the first
entry that is not polinrider's. A missing or empty folder is "nothing to
delete", exit 0.

It is its own command, spelled as 1.x spelled it. It takes `--evidence` and
refuses every other flag and any directory, because a purge that also
scanned, or that took a root, would be one typo away from a different
folder. It reads no indicator set.

`AGENTS.md` rule 1 now names this as the one exception.

## Consequences

- A cleanup can end with the copies gone, without anyone typing `rm -rf`.
- **It deletes the commits a restore may still need.** The screen says so
  before the question; nothing checks whether a restore is outstanding,
  because nothing records that between runs (ADR-0030). 1.x did warn about an
  outstanding restore, from state 2.0 no longer keeps.
- **The guard is strict.** A note or a download dropped into the folder
  blocks the purge until it is moved out. That is the intended cost: a false
  refusal is a nuisance, a false delete is not recoverable.
- There is no `--yes`. It cannot be scripted without feeding `yes` on
  standard input, which is deliberate.
- A run interrupted during the delete can leave part of the folder. The
  message says so, and running it again finishes the job.

## Related

- ADR-0003: evidence lives in a temporary directory.
- ADR-0030: 2.0 keeps no state between runs.
- ADR-0032: dry run by default; a typed yes for anything that changes state.
