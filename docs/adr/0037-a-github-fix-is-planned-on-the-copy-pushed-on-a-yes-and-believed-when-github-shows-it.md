# 0037. A GitHub fix is planned on the copy, pushed on a yes, and believed when GitHub shows it

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-03 |

## Context

ADR-0036 agreed four fixes for a repository on GitHub and built none of them:
`restore`, `erase`, `remove` and `archive`. The check could find the payload
and then pointed at 1.x for the repair.

1.x has all but `archive`, across `lib/gh-preserve.sh`, `lib/gh-restore.sh`
and `lib/gh-clean.sh`. Porting them raised questions the mock did not answer,
because the mock showed screens and these are about what happens between them.
This records how each was settled. Where the answer differs from 1.x, it says
so, and the maintainer has not yet ruled on those.

## Decision

**Every fix is a pair.** `plan_*` in `src/remote_fix.rs` reads the copy and
returns exactly what would change. It may fetch. It never pushes. The function
named after the fix takes that plan and does it, and the only caller is the
screen that has just shown the plan and read `yes`. All-at-once is the same
pair per repository, behind the owner's name.

**The four things that change GitHub are four methods on `Forge`**: `push`,
`set_description`, `archive`, and nothing else. `Supplied` implements them
against the bare repositories it already serves, so a test pushes for real
and then reads what the pretend GitHub holds.

**A push moves a branch only if it still points where the copy says.** Every
forced update carries `--force-with-lease=<branch>:<what the copy saw>`. A
push that landed between the check and the yes is never overwritten: the fix
is refused and says so. 1.x moved the branch through the API with
`force=true` and would have overwritten it.

**A push is not taken at its word.** After each one GitHub is asked where the
branches point, and only what it shows is reported as done. What it does not
show is named.

**What a fix moves away from is kept**, in the copy, under
`refs/polinrider/before-fix/<time>/`. The copy is then moved to match GitHub,
so a second check in the same session sees the repository as it now is. A
copy left from an earlier run is brought up to date before it is checked, for
the same reason.

**`restore` goes to the newest state on GitHub's record that still fetches
and checks clean.** The record is walked backwards from the latest push. A
`before` that carries the payload is an earlier wave and is skipped. A
`before` that GitHub no longer serves is skipped. The first clean one is the
target. Commit dates are never read.

1.x chose differently in two places. `gh-preserve.sh` recommends the
*earliest* clean commit on record, which throws away every legitimate push
between it and the attack. `gh-restore.sh` takes the commit before the first
push after a time the operator supplies, and refuses a target pushed inside
that window. The guided flow asks for no time, so the rule here has to work
without one, and "newest clean" is the rule that loses the least work.

What it costs: "clean" means clean against today's indicators. An earlier
wave carrying a variant nobody has an indicator for would be chosen as the
target. That is the same limit every verdict this tool gives has, and the dry
run shows which push is being undone, by whom and when, so that a person can
see a target that is too recent.

**`remove` cuts the payload out of a build config and keeps the file.** 1.x
deleted every flagged path, which removes `postcss.config.mjs` whole and
leaves a project that no longer builds. `strip` already knows the one shape
that is safe to cut (ADR-0031), so a config of that shape is rewritten
without the payload and anything else is deleted. The mock said "deletes the
payload files"; the dry run now says which it does to each file. The new
commit is checked before it is pushed, and one that still carries the payload
is not pushed.

**`erase` takes its list of paths from the whole history, not the newest
commit.** One pass reads every file version any branch or tag reaches. A
payload that once sat in a file since renamed or overwritten is in the
history and not at the tip, and 1.x, which removed the paths the tip showed,
would leave it and report it gone. The rewrite runs in a second copy; the
result is read the same way before anything is pushed, and a rewrite that
still holds the payload is thrown away.

Three smaller choices inside it. It uses `git filter-branch` only, which
ships with git, and not `git filter-repo` when installed as 1.x did: one
path, tested, and slow on a large repository. Commits left empty are kept,
where 1.x pruned them, because pruning can delete a branch outright. And
after the rewrite, the clean part of each build config is put back as one
commit, since the rewrite removed the file from every commit including the
ones where it was clean.

**`archive` puts a heading above the warning.** The notice opens with a
level-one heading in capitals, then GitHub's caution block with the warning
in bold. It goes above whatever the README holds, separated by a rule, and
nothing is removed. The date "on or after" is the earliest push GitHub
recorded on an infected branch, and when there is no record the sentence
says "ever" and gives no date. A second run does not stack a second notice.

**Two words were added to the choice after the summary.** `none`, to go to
the last screen without fixing anything, which the mock had no way to do
short of quitting. And with one repository the choice is `fix`, since `each`
and `all` mean the same thing there.

**The exit code is still 2 after a fix.** The check found a confirmed
indicator. A script reading the exit code must not be told nothing happened,
as with `clean` in the local flow.

## Consequences

Better: the second command ADR-0036 apologised for is gone. Finding and
fixing are one session. The corpus now guards the half that pushes: a case
has to name any repository it expects to change, and every other one must be
exactly as it was.

Worse:

- **Still nothing here has touched real GitHub.** Protected branches,
  rulesets, the real shape of `gh`'s errors, whether a lease holds through
  GitHub's push path, and whether GitHub serves an orphaned commit for as
  long as assumed are all untested. Trying it needs a network, a sign-in and
  a throwaway repository inside the sandbox.
- **`erase` reads every file version into memory to plan**, and rewrites with
  the slow tool. A large repository will take long enough to be mistaken for
  a hang, and the screen says only "this can take a while".
- **`erase` does not purge.** GitHub keeps the old commits, reachable by ID
  and from pull requests, until Support runs a garbage collection. The last
  screen says to ask. Forks keep their own copies regardless.
- **`restore` drops every commit pushed after its target**, including real
  work a colleague pushed from an infected clone. It is kept in the copy and
  the dry run counts it, but the copy lives in a temporary folder.
- **A partly restored repository still carries the payload** on the branches
  with no record, and the session cannot then fix those: it says to run
  again.
- **Tags are fixed only by `erase`.** `restore` and `remove` say a tag still
  carries the payload and leave it.
- **A commit made by a fix is unsigned**, authored by the operator's git
  identity or, when there is none, by the signed-in account at its GitHub
  noreply address. A repository that requires signed commits will refuse it.
- **Not built:** `everything` (this computer, then GitHub) and the progress
  screen for machine checks.

## Related

- ADR-0036, the agreed behaviour these implement.
- ADR-0011, pre-attack commits are fetched, not assumed present.
- ADR-0031, the one shape that is cut out of a file and not deleted.
- ADR-0032, only a typed yes changes anything.
