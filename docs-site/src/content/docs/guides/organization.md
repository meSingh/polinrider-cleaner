---
title: An organization
---

Run `polinrider` and answer `organization`. It checks every repository of an organization you belong to,
every branch and every tag.

:::caution
Check every machine and rotate every credential first.
[Order matters](/polinrider-cleaner/guides/order/).
:::

## Before it starts

It needs [`gh`](https://cli.github.com/), installed and signed in. It checks
that first and says what to run if not. It lists the organizations your `gh` sign-in can see and asks which one.

## What it does

1. **Copies every repository** into an evidence folder outside any git working
   tree, before anything changes. Those copies hold live malware, so do not
   open them in an editor.
2. **Checks every branch and tag** against the indicator set, and reads
   GitHub's push record for each repository.
3. **Shows a summary**: which repositories, how many branches, who pushed.
   `details` lists every branch and file.
4. **Asks how to go through them**: `each` for one repository at a time,
   `all` for one fix everywhere it applies, or `none` to change nothing.

## The fixes

| Choice | What it does |
|---|---|
| `restore` | Moves each branch back to the newest clean state on GitHub's push record. Offered only where that record shows one, and only after the commit has been fetched and checked clean. Never trusts commit dates, which this malware forges |
| `erase` | Rewrites history so the payload is in no commit, force-pushes, then shows how to reset every existing clone. Every commit ID on the rewritten branches changes |
| `remove` | One new commit that takes the payload out. History is left as it is |
| `archive` | Puts a large infected notice at the top of the README without removing anything, rewrites the description and makes the repository read-only. For a repository nobody uses |
| `skip` | Leaves it |

Each fix shows a dry run of exactly what would change on GitHub, and pushes
only when you type `yes`. Every push carries a lease, so a push that landed
since the check is never overwritten. Afterwards it asks GitHub what it now
shows and reports that, not what it intended.

`all` warns first, says which repositories can take the fix and which will be
left alone, and goes ahead only when you type the owner's name. `yes` is not
enough there.

## Which fix

If the branch was **force-pushed** to a rewritten history, GitHub's push record
still holds the earlier commit and `restore` goes back to it. If the payload
was **committed normally**, there is no earlier state to return to, so
`remove` or `erase` is the fix. The push record keeps roughly the last 300
events per repository, so run the check before anyone pushes a fix by hand.

After any fix, delete every local clone and clone again. A `git pull` into an
infected clone can re-infect the remote.
