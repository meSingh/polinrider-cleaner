# 0036. GitHub is checked through one boundary, and fixed one agreed way at a time

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-03 |

## Context

1.x checks and repairs GitHub: an organization's repositories or an account's.
It is about 1,500 lines of shell across two near-duplicate folders, and it is
the part ADR-0026 most wanted gone. The Rust engine had none of it, and the
guided flow of ADR-0035 offered only "computer" and "folder". The maintainer
tried that flow, liked it, and missed the GitHub choices at once.

Two things make this harder than the machine check. Everything it does to
GitHub is public and hard to take back: a force-push cannot be quarantined.
And it cannot be tested where everything else is tested, because the sandbox
has no network and no sign-in.

The screens were drawn as a mock and went through three rounds with the
maintainer before any of them was built. What follows is what was agreed.

## Decision

**GitHub is asked through one trait, `Forge`**, in `src/remote.rs`, as the
machine is asked through `Host`. `GitHub` is the real thing, through `gh` and
`git`. `Supplied` answers from a directory of bare repositories and text
files, loaded with `--forge-state`. Tests and the corpus build an organization
with an infected branch and assert on what is found, with no network.

**Repositories are checked without being checked out.** Each is
mirror-cloned into an evidence directory and read through git plumbing. The
directory defaults to the system's temporary folder, so forgetting about
infected mirrors is the safe outcome, and one inside a git checkout is
refused.

**Sign-in is settled before anything else.** If `gh`, GitHub's own CLI, is
missing or signed out, a screen says what to run for that system and checks
again on Enter.

**Organizations are listed**, with their repository counts. A name is typed
from the list; one not on it still works.

**One progress screen for every long job**: a bar, what it is on now, what it
has found so far, how long it has run. Redrawn in place on a terminal; a pipe
gets the finished state once.

**The fixes, each on the screen only once it works:**

- `restore` puts a branch back where **GitHub's own push record** says it was
  before the attack, after that commit has been fetched and checked clean. It
  is never "the last commit" or anything read from commit dates: this malware
  backdates and amends, and a commit's date is whatever its author said. The
  push record is made by GitHub. When the record is gone, the screen says so.
- `erase` takes the payload out of every commit and pushes the history back.
  Every commit ID changes and every clone has to be reset to the remote, so
  the screen after it says how, with the real branch names.
- `remove` adds one commit per branch deleting the files. Kept from 1.x as the
  gentlest option; the payload stays in history.
- `archive` is new: for a repository nobody uses. A notice goes at the top of
  the README, large and above whatever is already there, removing nothing; the
  description is replaced with a warning; and the repository is archived.

**All at once is possible and is made deliberate.** One fix for every affected
repository, after a screen that says how many repositories and branches change
and which will be left alone, confirmed by typing the organization's name.

**A dry run before every push**, on the mirror, showing exactly what would
change, and then one question.

Built so far: the boundary, the check, sign-in, the organization list, the
progress screen, the summary and the last screen. The fixes are next.

## Consequences

Better: the largest piece of shell left has a tested replacement under way,
and the dangerous half of it is being built behind screens somebody approved.

Worse:

- **None of this has touched real GitHub.** Every test runs against local
  repositories. `gh`'s real output, real rate limits, a real organization with
  a thousand repositories and a real protected branch are all unseen. The
  first real run needs a network, a sign-in and a throwaway repository inside
  the sandbox, which only the maintainer can provide.
- **Until the fixes land, the last screen points at 1.x** for the repair. That
  is a second command, which ADR-0026 said there would not be.
- **Copying every repository is slow and large.** An organization with
  hundreds of repositories means hundreds of mirror clones before the first
  finding. 1.x has a faster pre-check, the sweep of push events, that narrows
  the list first. It is not ported, and until it is the progress screen is an
  apology for the wait.
- **Three places differ from 1.x on purpose**, on top of those in ADR-0029.
  The token is no longer written into the clone URL. A `tasks.json` that runs
  on folder open with no indicator is review, not confirmed. And a path under
  `lib/` or `ci/` is no longer assumed to be the operator's own tooling.
- **`archive` leaves the malware in place.** It warns and locks. Anyone who
  clones the repository still gets the payload. The screen says so, and people
  will read "archived" as "dealt with".
- **Typing an organization's name is friction on the one action people will
  most want to hurry.** That is the intent. It will still be called annoying.

## Related

- ADR-0029, host state through one boundary: the same shape, for the same
  reason.
- ADR-0035, four calm screens: the rules these screens follow.
- ADR-0009, a known actor escalates rather than dismisses: why a pusher's name
  is listed and never used to discount a finding.
- ADR-0011, pre-attack commits are fetched, not assumed present: what
  `restore` has to do before it can offer anything.
