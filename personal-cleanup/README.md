# Personal profile cleanup

Restore the branches on your own GitHub account after your credentials were used
to force-push. History stays intact and no work is lost: the branch pointer moves
back to the commit that existed before the attack, and the malicious commits
become unreachable.

**This is not the org workflow with a different flag.** On a personal account the
hostile pushes carry *your* login, because they were made with your stolen token.
Filtering by actor proves nothing. Your evidence is the time window and the
signature of the pushes, and your first job is to make your own credentials
useless to whoever took them.

**Scripts in this folder**

| Script | What it does | Changes anything? |
|---|---|---|
| `scan.sh` | Mirror-clones every repo you own, captures push events, scans every ref | No |
| `sweep.sh` | Lists every ref-touching event since T0 | No |
| `triage-filter.sh` | Separates real findings from your own detection tooling | No |
| `restore.sh` | Builds the restore plan, and with `--apply` performs the restore | Only with `--apply` |
| `preflight.sh` | Credential and safety gates before `--apply` | No |

---

## Step 1 — clean the machine, then lock yourself out of your own account

In this order, and finish both before touching a single branch.

**1. The machine.** Run [`../local-cleanup/`](../local-cleanup/) on every machine
you have used for git. If anything comes back as a confirmed hit, that machine is
out of the process entirely — do the rest from a different one.

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

Anything else that was on the machine — npm tokens, cloud keys, `.env` values,
browser passwords, crypto wallets — see
[root README, section 4](../README.md#4-credential-rotation). If a wallet or seed
phrase was on that machine, move the funds now.

---

## Step 2 — capture evidence

Time-critical. The pre-attack commit SHAs come from the Events API, which keeps
roughly the last 300 events per repository. Every push you make brings the
attacker's push closer to falling off the end.

```bash
./scan.sh --user YOUR-USERNAME --out ./evidence --mirror-only
```

Add `--no-forks` to skip forks. Keep `./evidence/` until you are done.

If you own a lot of repositories, run the sweep in step 3 first and then narrow
the mirroring to what it found:

```bash
./scan.sh --user YOUR-USERNAME --out ./evidence --mirror-only \
          --from-sweep ./evidence/sweep.tsv
```

---

## Step 3 — find the hostile pushes

```bash
./sweep.sh --user YOUR-USERNAME --since 2026-07-27T03:00:00Z --out ./evidence
```

Set `--since` about two hours before the earliest push you know you did not make.

Now read the output and mark every push you actually made. What is left is the
attacker. Three signatures make this easy:

- **many repositories in one narrow window** — you do not push to fourteen repos
  in ninety seconds
- **every branch in a repo moved to the same commit**, `size` column `0` — one
  malicious commit, every ref pointed at it
- **branches you have not touched in months** suddenly moving

Then look at the content:

```bash
./scan.sh --user YOUR-USERNAME --out ./evidence --scan-only
./triage-filter.sh ./evidence/triage.json
cat ./evidence/triage.txt
```

Read the matched lines. A count is not evidence.

---

## Step 4 — build the restore plan

Dry run. Changes nothing. No `--actor`: on a personal account it would match
everything.

```bash
./restore.sh --sweep ./evidence/sweep.tsv --mirrors ./evidence \
             --since 2026-07-27T03:00:00Z
```

Writes `evidence/restore-plan.tsv`. The statuses are explained in the
[org-cleanup README, step 5](../org-cleanup/README.md#step-5--build-the-restore-plan).
Two of them matter most here:

- `ok_orphaned` is **normal and good**. The commit is unreachable from any ref,
  so your mirror never fetched it, but GitHub still holds it and confirmed so.
  It does not mean work was lost.
- `MALICIOUS_TARGET` means the target is itself a commit from the attack window.
  `restore.sh` refuses those rows. If you see any, your `--since` starts too late:
  move it earlier and rebuild the plan.

**Read every row before applying.** This is the step where your own work can be
lost: any commit you pushed *after* the hostile push on that branch is orphaned by
the restore. Restrict the plan to one repository with `--repo YOU/REPO` while you
work through them if that is easier.

---

## Step 5 — preflight

```bash
./preflight.sh --plan ./evidence/restore-plan.tsv
```

It lists the SSH and signing keys currently on your account — delete any you do
not recognise — checks that no restore target is a commit from the attack window,
prints one diff per before→after pair so you can confirm the payload, and lists
what the restore would orphan.

---

## Step 6 — restore

```bash
./restore.sh --sweep ./evidence/sweep.tsv --mirrors ./evidence \
             --since 2026-07-27T03:00:00Z --apply
```

If a restore returns 422, a ruleset or branch protection on that repository is
blocking it. Disable it, restore, re-enable it.

Recover any of your own work that the restore orphaned by cherry-picking from the
mirror in `./evidence/`:

```bash
git -C ./evidence/REPO.git log --oneline <orphaned-sha> -20
```

---

## Step 7 — verify, then re-clone

```bash
rm -rf ./evidence-post
./scan.sh --user YOUR-USERNAME --out ./evidence-post
./triage-filter.sh ./evidence-post/triage.json
```

Expect zero `REAL_SUSPECT`.

Then delete every local clone and clone fresh. A `git pull` into an infected clone
pushes the payload straight back to the remote you just cleaned.

Finally, turn on the protections that stop the next one — required signed commits
and blocked force pushes on your repositories, and the scan workflow in
[`../ci/`](../ci/). Re-scan weekly for a month: reinfection with a rotated
signature is documented behaviour for this campaign.
