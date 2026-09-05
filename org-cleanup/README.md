# Organization cleanup

Restore every force-pushed branch across a GitHub organization to the commit that
existed before the attack. History stays intact and no work is lost: the branch
pointer moves back, and the malicious commits become unreachable.

Read [step 0](#step-0--before-anything-else) before running anything. It decides
whether this method can work for you at all.

**Scripts in this folder**

| Script | What it does | Changes anything? |
|---|---|---|
| `scan.sh` | Mirror-clones every repo, captures push events, scans every ref for indicators | No |
| `sweep.sh` | Lists every ref-touching event since T0, so you can name the attacker's pushes | No |
| `triage-filter.sh` | Separates real findings from your own detection tooling matching itself | No |
| `restore.sh` | Builds the restore plan, and with `--apply` performs the restore | Only with `--apply` |
| `preflight.sh` | Five gates that must pass before `--apply` | No |

---

## Step 0 — before anything else

**Is anyone's machine still infected?** Run [`../local-cleanup/`](../local-cleanup/)
on every developer machine first. Restoring branches while an implant holds a live
token means it re-pushes within minutes. This campaign did exactly that in one
documented case: a second wave landed fifty minutes after the first containment.

**Can the pre-attack SHAs still be recovered?** They come from the per-repository
Events API, which keeps roughly the last 300 events. Git push events appear in the
organization audit log only on GitHub Enterprise Cloud. Check now, on your busiest
repository:

```bash
gh api "/repos/ORG/REPO/events?per_page=100" --paginate \
  | jq -r '.[] | select(.type=="PushEvent")
           | [.created_at, .actor.login, .payload.ref,
              (.payload.before[0:10]), (.payload.head[0:10])] | @tsv' \
  | sort | head -5
```

Read the oldest timestamp:

- **older than the attack** → this method works. Continue.
- **newer than the attack** → the attacker's pushes have aged out of that
  repository. Restore is no longer possible there; delete the affected branches
  and recreate them from a clean default branch instead.

Every push anyone makes shortens this window. Do step 2 today.

---

## Step 1 — freeze

Branch protection on `main` is not a freeze. The malware creates branches too.

```
Organization → Settings → Repository → Rulesets → New ruleset
  Name        IR-FREEZE
  Enforcement Active
  Target      All repositories, all branches
  Rules       Restrict creations, Restrict updates,
              Restrict deletions, Block force pushes
  Bypass list your account only
```

Also disable Actions organization-wide until you have verified the repos. An
infected branch can carry a workflow file, and re-enabling pushes would run it
with your secrets.

If org rulesets are not available on your plan, apply the same rules per
repository with a `**` branch pattern.

Then cut off the compromised account. Removing an SSH key is not enough: the
stealer took session cookies, tokens and cloud keys.

```bash
gh api -X DELETE /orgs/ORG/memberships/USERNAME
```

Revoke their PATs, OAuth grants and GitHub Apps in the UI, and force a password
reset with global sign-out on your identity provider.

---

## Step 2 — capture evidence (blocking)

Nothing gets restored before this finishes. Cleanup is reversible; losing the
pre-attack SHAs is not.

```bash
./scan.sh --org YOUR-ORG --out ./evidence --mirror-only
```

This mirror-clones every repository, freezes garbage collection on each mirror so
unreachable commits survive, and writes the push ledger to `evidence/pushes.tsv`.

**On a large organization, do step 3's sweep first and narrow this.** The sweep
is API-only and costs nothing; mirroring two hundred repositories costs disk and
hours. Once you have `sweep.tsv`, clone only what was touched:

```bash
./sweep.sh --org YOUR-ORG --since <T0> --out ./evidence      # cheap, API only
./scan.sh  --org YOUR-ORG --out ./evidence --mirror-only --from-sweep ./evidence/sweep.tsv
```

The trade-off is real and you should make it deliberately: repositories outside
the sweep window are not mirrored, so if the events API has already dropped an
older attack you will not see it. Mirror everything if you can afford to.

On GitHub Enterprise Cloud, export the audit log too:

```bash
gh api --paginate "/orgs/YOUR-ORG/audit-log?phrase=action:git.push&include=all" \
  > ./evidence/audit-log-git-push.json
```

Keep `./evidence/` until the incident is closed. It is your forensic baseline.

---

## Step 3 — establish scope

Two views. Run both.

**What was touched, and by whom.** Set `--since` about two hours before the first
push you believe was malicious:

```bash
./sweep.sh --org YOUR-ORG --since 2026-07-27T03:00:00Z --out ./evidence
```

Every `PushEvent` in the output needs a named person who will say "yes, that was
me, at that time, on that branch". Anything unclaimed is the attacker.

A `CreateEvent` for a branch inside the window is an attacker artifact. A restore
cannot help there, because the branch has no earlier state to return to. `sweep.sh`
prints a ready-to-run `gh api -X DELETE` line for each one; confirm with the
supposed author, then run the ones nobody claims.

If every branch in a repository moved to the same commit with `size 0`, the
malware made one commit and pointed every ref at it. You then need to read one
diff per repository, not one per branch.

**What the content looks like now:**

```bash
./scan.sh --org YOUR-ORG --out ./evidence --scan-only
cat ./evidence/triage.txt
```

---

## Step 4 — filter the false positives

A grep-based scanner cannot tell a file that *is* the malware from a file that
*detects* it. Your own scan workflows will be flagged.

```bash
./triage-filter.sh ./evidence/triage.json
```

`BENIGN_TOOLING` lines are your own detection code. `REAL_SUSPECT` lines need
your eyes. Read the matched content in `triage.txt` — never act on a count.

A `clean` verdict means the current indicator set is absent from that ref. It is
not proof the ref was never touched. That is what step 3 is for.

---

## Step 5 — build the restore plan

Dry run. Changes nothing.

```bash
./restore.sh --sweep ./evidence/sweep.tsv --mirrors ./evidence \
             --since 2026-07-27T03:00:00Z --actor ATTACKER-LOGIN
```

Writes `evidence/restore-plan.tsv`, one row per branch:

| Status | Meaning | Restorable |
|---|---|---|
| `ok` | That push rewrote history. Definite force push | yes |
| `ok_fastforward` | Commits were appended without a rewrite. Read the diff first — a legitimate push in the window looks identical | yes, after review |
| `ok_orphaned` | The target commit is unreachable from any ref, so the mirror never fetched it, but GitHub still holds it and confirmed so. **This is the normal state for a branch the attacker moved. It does not mean work was lost** | yes |
| `MALICIOUS_TARGET` | The target is itself a commit pushed during the attack. This is the second-wave trap | no — widen `--since` and rebuild the plan |
| `SHA_GONE` | The commit returned 404. It has been garbage collected | no — delete and recreate the branch |
| `NO_MIRROR` | No local mirror. Re-run step 2 for that repository | no |

**The second-wave trap.** If the attacker pushed twice, the second push's `before`
value is the first push's malicious commit. Restoring to it would pin your
branches to malware. `restore.sh` refuses those rows outright, but if you see any,
your `--since` starts too late. Move it earlier and rebuild the plan.

---

## Step 6 — preflight

```bash
./preflight.sh --org YOUR-ORG --plan ./evidence/restore-plan.tsv --actor ATTACKER-LOGIN
```

It blocks if the attacker is still an org member, and if any restore target is a
commit from the attack window. It also prints one diff per distinct before→after
pair, so you can confirm the payload with your own eyes, and lists the branches
whose recent commits a restore would orphan.

Exit 1 means blocked. Fix the cause; do not proceed.

---

## Step 7 — restore

Same command as step 5 with `--apply`:

```bash
./restore.sh --sweep ./evidence/sweep.tsv --mirrors ./evidence \
             --since 2026-07-27T03:00:00Z --actor ATTACKER-LOGIN --apply
```

Each restore is one API call: `PATCH /repos/OWNER/REPO/git/refs/heads/BRANCH`
with `force=true`. If a call returns 422, your account is not in the ruleset
bypass list from step 1.

Legitimate work pushed *after* the attacker's push on a branch becomes orphaned.
It is still in the mirror under `./evidence/`. Cherry-pick it forward afterwards,
commit by commit, reading each diff.

Restoring makes the malicious commit unreachable, not deleted. It stays in
GitHub's object store until garbage collection. That is fine operationally. If
you need it provably gone — audit, insurance, customer contract — open a GitHub
Support ticket, or rebuild the repository from the clean mirror.

---

## Step 8 — verify

```bash
rm -rf ./evidence-post
./scan.sh --org YOUR-ORG --out ./evidence-post
./triage-filter.sh ./evidence-post/triage.json
```

Expect zero `REAL_SUSPECT`. Then confirm nothing has been pushed since your
restore:

```bash
./sweep.sh --org YOUR-ORG --since <time-of-your-restore> --out ./evidence-post
```

An empty sweep is the evidence that there was no third wave.

---

## Step 9 — reopen

Do not lift `IR-FREEZE` until the hardening in the
[root README, section 5](../README.md#5-hardening) is in place, starting with
**required commit signing** — the direct counter to the backdated-amend technique.

Then, per repository: lift the freeze, re-enable Actions, and tell the team they
may work on it again **from a fresh clone**. Existing local clones stay untrusted.

Finally, add the scan to CI so the next attempt is caught on the push that makes
it: [`../ci/`](../ci/).
