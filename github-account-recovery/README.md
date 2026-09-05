# GitHub account recovery

<sub>[← back to the main README](../README.md) · this folder is for **one personal
GitHub account**. For an organization use [`github-org-recovery/`](../github-org-recovery/); for a
computer use [`machine-cleanup/`](../machine-cleanup/).</sub>

---

**What this does.** Puts the branches on your own account back to the commit that
existed before your credentials were used to force-push.

**What "cleanup" means here.** Not deleting. The branch pointer moves back to a
commit that still exists. History is intact and no work is lost.

> [!IMPORTANT]
> **This is not the org workflow with a different flag.** On a personal account
> the hostile pushes carry *your* login, because they were made with your stolen
> token. Filtering by actor proves nothing here. Your evidence is the time window
> and the shape of the pushes. Your first job is to make your own
> credentials useless to whoever took them.

---

> [!TIP]
> **Just want to know whether you were hit?** From the repository root:
>
> ```bash
> ./polinrider.sh --user YOUR-USERNAME
> ```
>
> Read-only, and it stops before anything can change. The rest of this page is
> the recovery, which you drive yourself on purpose.

> [!IMPORTANT]
> The steps on this page change state on GitHub. This tool comes with no
> warranty and no liability, and it assumes you are working on your own account.
> See [DISCLAIMER.md](../DISCLAIMER.md).

## The scripts

| Script | What it does | Changes anything? |
|---|---|---|
| `scan.sh` | Mirror-clones every repo you own, captures push events, scans every ref | No |
| `sweep.sh` | Lists every ref-touching event since T0 | No |
| `triage-filter.sh` | Separates real findings from your own detection tooling | No |
| `restore.sh` | Builds the restore plan, and with `--apply` performs the restore | Only with `--apply` |
| `preflight.sh` | Credential and safety gates before `--apply` | No |
| `clean-repo.sh` | Removes a committed payload from every affected branch of one repo | Only with `--apply` |

---

## Step 1. Clean the machine, then lock yourself out of your own account

In this order, and finish both before touching a single branch.

**1. The machine.** Run `./polinrider.sh --machine` on every machine you have
used for git. See [`../machine-cleanup/`](../machine-cleanup/). If anything comes
back as a confirmed hit, that machine is out of the process entirely; do the rest
from a different one.

**2. The credentials.** From a machine you trust:

| What | Where |
|---|---|
| Every personal access token, classic and fine-grained | <https://github.com/settings/tokens> |
| Every SSH key **and signing key** | <https://github.com/settings/keys> |
| Authorised OAuth apps | <https://github.com/settings/applications> |
| Password change + "sign out of all other sessions" | <https://github.com/settings/security> |
| Deploy keys on your repos | `gh api /repos/YOU/REPO/keys` |

Then clear the cached credentials that survive a password change:

```bash
gh auth logout && rm -f ~/.config/gh/hosts.yml
git credential-osxkeychain erase <<< $'protocol=https\nhost=github.com'   # macOS
cmdkey /delete:git:https://github.com                                     # Windows
```

Then authenticate again with a fresh token: `gh auth login`.

Anything else that was on the machine, including npm tokens, cloud keys, `.env`
values, browser passwords and crypto wallets: see
[the rotation list in the main README](../README.md#step-2-rotate-every-credential).

> [!CAUTION]
> If a crypto wallet or seed phrase was on that machine, move the funds now. This
> malware targets them specifically.

---

## Step 2. Capture evidence

> [!CAUTION]
> Time-critical. The pre-attack commit SHAs come from the Events API, which keeps
> roughly the last 300 events per repository. Every push you make brings the
> attacker's push closer to falling off the end.

```bash
./scan.sh --user YOUR-USERNAME --out ~/.polinrider/evidence --mirror-only
```

Add `--no-forks` to skip forks. Keep `~/.polinrider/evidence/` until you are done.

If you own a lot of repositories, run the sweep in step 3 first and then narrow
the mirroring to what it found:

```bash
./scan.sh --user YOUR-USERNAME --out ~/.polinrider/evidence --mirror-only \
          --from-sweep ~/.polinrider/evidence/sweep.tsv
```

---

## Step 3. Find the hostile pushes

```bash
./sweep.sh --user YOUR-USERNAME --since 2026-07-27T03:00:00Z --out ~/.polinrider/evidence
```

Set `--since` about two hours before the earliest push you know you did not make.

Now read the output and mark every push you actually made. What is left is the
attacker. Three signatures make this easy:

- **many repositories in one narrow window.** You do not push to fourteen repos
  in ninety seconds
- **every branch in a repo moved to the same commit**, `size` column `0`. One
  malicious commit, every ref pointed at it
- **branches you have not touched in months** suddenly moving

Then look at the content:

```bash
./scan.sh --user YOUR-USERNAME --out ~/.polinrider/evidence --scan-only
./triage-filter.sh ~/.polinrider/evidence/triage.json
cat ~/.polinrider/evidence/triage.txt
```

Read the matched lines. A count is not evidence.

---

## Step 4. Build the restore plan

Dry run. Changes nothing. No `--actor`: on a personal account it would match
everything.

```bash
./restore.sh --sweep ~/.polinrider/evidence/sweep.tsv --mirrors ~/.polinrider/evidence \
             --since 2026-07-27T03:00:00Z
```

Writes `evidence/restore-plan.tsv`. The statuses are explained in the
[github-org-recovery README, step 5](../github-org-recovery/README.md#step-5-build-the-restore-plan).
Two of them matter most here:

- `ok_orphaned` is **normal and good**. The commit is unreachable from any ref,
  so your mirror never fetched it, but GitHub still holds it and confirmed so.
  It does not mean work was lost.
- `MALICIOUS_TARGET` means the target is itself a commit from the attack window.
  `restore.sh` refuses those rows. If you see any, your `--since` starts too late:
  move it earlier and rebuild the plan.

> [!WARNING]
> **Read every row before applying.** This is the step where your own work can be
> lost: any commit you pushed *after* the hostile push on that branch is orphaned
> by the restore. Restrict the plan to one repository with `--repo YOU/REPO` while
> you work through them if that is easier.

---

## Step 5. Preflight

```bash
./preflight.sh --plan ~/.polinrider/evidence/restore-plan.tsv
```

It lists the SSH and signing keys currently on your account, so you can delete any
you do not recognise. It checks that no restore target is from the attack window,
prints one diff per before→after pair so you can confirm the payload, and lists
what the restore would orphan.

---

## Step 6. Restore

```bash
./restore.sh --sweep ~/.polinrider/evidence/sweep.tsv --mirrors ~/.polinrider/evidence \
             --since 2026-07-27T03:00:00Z --apply
```

If a restore returns 422, a ruleset or branch protection on that repository is
blocking it. Disable it, restore, re-enable it.

Recover any of your own work that the restore orphaned by cherry-picking from the
mirror in `~/.polinrider/evidence/`:

```bash
git -C ~/.polinrider/evidence/REPO.git log --oneline <orphaned-sha> -20
```

---

## Step 7. Verify, then re-clone

```bash
rm -rf ~/.polinrider/evidence-post
./scan.sh --user YOUR-USERNAME --out ~/.polinrider/evidence-post
./triage-filter.sh ~/.polinrider/evidence-post/triage.json
```

Expect zero `REAL_SUSPECT`.

> [!WARNING]
> Then delete every local clone and clone fresh. A `git pull` into an infected
> clone pushes the payload straight back to the remote you just cleaned.

Finally, turn on the protections that stop the next one: required signed commits
and blocked force pushes on your repositories, and the scan workflow in
[`../ci/`](../ci/). Re-scan weekly for a month: reinfection with a rotated
signature is documented behaviour for this campaign.

---

## If there is nothing to restore to

`restore.sh` moves a branch pointer back to a commit that still exists in the
mirror. That only works when the branch was force-pushed. When the payload was
committed normally, or when the force-push is older than the roughly 90 days of
events GitHub keeps, there is no earlier state and `restore.sh` has nothing to
do.

The fix there is to remove the files and commit that removal:

```bash
# Dry run. Lists every branch and every file it would touch.
./clean-repo.sh OWNER/REPO

# Do it.
./clean-repo.sh OWNER/REPO --apply
```

It reads `affected-paths.tsv` and `affected-refs.tsv`, both written into your
evidence directory by the scan. It works in a bare clone using git plumbing, so
nothing is ever checked out and the payload never lands on your disk as a live
file. It adds one ordinary commit per branch and pushes normally. Nothing is
rewritten, nothing is force-pushed, and you can revert it.

A protected branch will reject the push. Clean an unprotected branch and open a
pull request from it.

`NEXT-STEPS.md`, written into your evidence directory after every scan, already
contains these commands with your repository names filled in.
