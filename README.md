# polinrider-cleaner

Detect and clean up after the **PolinRider** supply-chain campaign: on a GitHub
organization, on a personal GitHub account, and on a macOS, Linux or Windows
workstation.

Shell only. No Node, no Python, no packages to install, no third-party action in
your CI. Every destructive step is a dry run first.

---

## Start here

Answer one question: **what are you cleaning?**

| Situation | Go to |
|---|---|
| Branches across an organization's repos were force-pushed | [`org-cleanup/`](org-cleanup/) |
| Your own GitHub repositories were force-pushed | [`personal-cleanup/`](personal-cleanup/) |
| Your laptop opened an infected repo, or you installed a flagged package | [`local-cleanup/`](local-cleanup/) |
| You want every future push and PR scanned automatically | [`ci/`](ci/) |

If more than one applies, the order is fixed and it matters:

```
1. local-cleanup    on every affected machine   (a live implant re-pushes your cleanup)
2. revoke credentials                           (a live token re-pushes your cleanup)
3. org-cleanup or personal-cleanup              (restore the branches)
4. ci                                           (catch the next attempt)
```

Cleaning the remote while an infected machine still holds a valid token puts you
back where you started within minutes. That is not theoretical; it is the
documented reinfection behaviour of this campaign.

---

## Requirements

| Tool | Needed by | Install |
|---|---|---|
| `git` | everything | already present on a developer machine |
| `gh` (GitHub CLI, authenticated) | org and personal cleanup | `brew install gh` / [cli.github.com](https://cli.github.com) |
| `jq` | org and personal cleanup | `brew install jq`, `apt install jq`, `dnf install jq` |
| `bash` 3.2+ | macOS, Linux | already present |
| Windows PowerShell 5.1 | Windows local check | ships with Windows 10 and 11 |

The local checks and the CI scanner need only `git` and standard shell tools.
`gh` and `jq` are needed only where GitHub's API is involved.

```bash
git clone https://github.com/meSingh/polinrider-cleaner.git
cd polinrider-cleaner
chmod +x org-cleanup/*.sh personal-cleanup/*.sh local-cleanup/*.sh ci/*.sh lib/*.sh
```

Use a fresh, short-lived, fine-grained token created on a machine you trust, and
revoke it when you are done.

---

## 1. What PolinRider is

A supply-chain campaign attributed to DPRK-linked actors, tracked alongside the
Contagious Interview / Famous Chollima cluster. It was first observed in December
2025 and first documented publicly in March 2026. It is still active.

It is **not** repository defacement. The repository changes are how it travels.
The goal is credentials.

### How you get infected

| Entry point | What happens |
|---|---|
| A malicious npm, Go or Composer package | `postinstall` runs, or your build imports the poisoned module |
| A malicious VS Code / Cursor extension | runs the moment the editor loads it |
| A "take-home interview project" repository | you open it in your editor to review it |
| An already-infected repo you cloned | `.vscode/tasks.json` runs a command when the folder opens |

The last one is the important one. **Opening a folder is enough.** You do not
have to run the project, install anything, or click anything.

### What it does once it runs

The visible layer is an obfuscated JavaScript loader. The loader is a blockchain
dead-drop resolver: it reads an encrypted second stage from TRON, Aptos or BNB
Smart Chain, XOR-decrypts it, and `eval()`s it in memory. Because the payload
lives in blockchain transactions, there is no C2 domain to take down and blocking
one host achieves nothing.

The second stage has been the **DEV#POPPER** remote access trojan and
**OmniStealer**; earlier waves carried **BeaverTail**. What they take:

- browser session cookies and saved passwords
- GitHub personal access tokens, SSH keys, `gh` CLI credentials
- npm, cloud (AWS/GCP), database and CI platform tokens
- every value in every `.env` file it can read
- cryptocurrency wallets and seed phrases, which it targets specifically

### How it spreads to your repositories

A propagation script, `temp_auto_push.bat`, does this on the infected machine:

1. reads the last commit's author and timestamp
2. **sets the system clock back** to that timestamp
3. amends the commit with the payload, committing with `--no-verify`
4. restores the clock and force-pushes with `-uf` to every writable remote

The result looks like an ordinary, correctly dated commit. `git log` will not
show you anything wrong. Socket observed May 2026 compromises carrying January
2026 commit dates.

This is why detection has to be **content scanning plus push-event
reconciliation**, never history reading.

### Where the payload hides

| Artifact | Detail |
|---|---|
| Build config files | appended after the real `export default` / `module.exports`, behind roughly 280 spaces of padding: `postcss.config.mjs`, `tailwind.config.js`, `eslint.config.mjs`, `next.config.mjs`, `babel.config.js`, `vite.config.js`, `app.js` |
| Fake fonts | `.woff2` files under `public/`, `static/`, `assets/` whose bytes are JavaScript, not a font |
| Editor tasks | `.vscode/tasks.json` with `"runOn": "folderOpen"` running `curl … \| bash` |
| Propagation script | `temp_auto_push.bat` |
| Dependencies | `tailwindcss-style-animate`, `tailwind-mainanimation`, `tailwind-autoanimation`, `tailwindcss-typography-style`, `tailwindcss-style-modify` |

Two obfuscator variants are in circulation: the original marked `rmcej%otb%`
with function `_$_1e42`, and a newer one marked `Cot%3t=shtP` with function
`MDy`. Signatures rotate, so a clean scan proves the *current* indicator set is
absent — nothing more.

### Scale

| Date | Reported by | Figure |
|---|---|---|
| 8 Mar 2026 | OpenSourceMalware | 675 repositories, 352 owners |
| 11 Apr 2026 | OpenSourceMalware | 1,951 repositories, 1,047 owners |
| 2 Jul 2026 | Socket / The Hacker News | 108 packages, 162 artifacts: 61 Go, 19 npm, 10 Composer, 1 Chrome extension |
| 1 Aug 2026 | Socket tracking page | 121 packages, 196 artifacts |

Counts differ slightly between sources because they count different things.
Check the [live tracking page](https://socket.dev/supply-chain-attacks/polinrider)
before quoting a number.

---

## 2. How to tell whether you are affected

Three checks, in increasing cost. Run all three.

**On your machine, 2 minutes, changes nothing:**

```bash
./local-cleanup/check-macos.sh ~/Sites ~/Projects      # or check-linux.sh
```

**On one repository, 30 seconds, changes nothing:**

```bash
./ci/scan-workspace.sh --path /path/to/repo --all-refs
```

**On GitHub, 10 minutes, changes nothing:**

```bash
./org-cleanup/scan.sh --org YOUR-ORG --out ./evidence
# then, because a scanner flags its own detection files:
./org-cleanup/triage-filter.sh ./evidence/triage.json
```

Read what matched, not the count. `cat ./evidence/triage.txt`.

Also check by eye, because no scanner covers these:

```bash
git reflog                      # amended commits on branches you own
gh api /user/keys               # SSH keys you did not add
```

On GitHub itself, look at the branch activity, not the commit list. A force push
appears as "*force-pushed the branch from `abc123` to `def456`*". A backdated
amend appears nowhere else.

---

## 3. If you are infected

The full sequence, with the folder that covers each step.

| # | Step | Where |
|---|---|---|
| 1 | Stop pushing. Freeze the org or your repos so the implant cannot re-push | [`org-cleanup/`](org-cleanup/) step 1 |
| 2 | Check and clean every affected machine | [`local-cleanup/`](local-cleanup/) |
| 3 | Rotate every credential, from a machine you trust | [section 4](#4-credential-rotation) |
| 4 | Capture evidence **before** touching any branch | [`org-cleanup/`](org-cleanup/) step 2 |
| 5 | Establish scope: which repos, which refs, which pushes | [`org-cleanup/`](org-cleanup/) step 3 |
| 6 | Restore each branch to its pre-attack commit | [`org-cleanup/`](org-cleanup/) steps 5–7 |
| 7 | Re-scan and confirm | [`org-cleanup/`](org-cleanup/) step 8 |
| 8 | Harden, then reopen | [section 5](#5-hardening) |
| 9 | Add the scan to CI and re-scan weekly | [`ci/`](ci/) |

Step 4 is time-critical and blocks everything after it. The pre-attack commit
SHAs come from the GitHub Events API, which keeps roughly the last 300 events per
repository. Every push anyone makes moves the attacker's push closer to falling
off the end. Once it is gone, non-destructive restore is no longer possible for
that branch.

### 4. Credential rotation

Assume everything reachable from the infected user account is in someone else's
hands. Rotate in this order, all from a clean machine.

**First — these grant repository write:**

- every GitHub personal access token, classic and fine-grained
- every SSH key **and signing key**
- **deploy keys on every repository** — routinely missed: `gh api /repos/OWNER/REPO/keys`
- authorised OAuth apps and GitHub Apps
- Actions secrets and variables, at org, repo and environment level
- self-hosted runner registration tokens; rebuild self-hosted runners from image

**Second — these grant publish rights:**

- npm tokens, plus 2FA reset. Check `npm token list` for tokens you did not create
- Packagist, PyPI, Go proxy, Docker Hub / GHCR, Chrome Web Store credentials

**Third — everything else the stealer could read:**

- cloud keys: `~/.aws/credentials`, `~/.config/gcloud`, service account keys
- every value in every `.env` on the affected machine
- database, Redis and broker passwords
- Slack, Stripe, Twilio, SendGrid and payment gateway keys
- Vault tokens and kubeconfigs
- browser-saved passwords, and session cookies via a forced global sign-out
- **crypto wallets. If a hot wallet or seed phrase was on that machine, move the
  funds now.** This malware targets them specifically.

Also clear cached git credentials, which survive a password change:

```bash
# macOS
git credential-osxkeychain erase <<< $'protocol=https\nhost=github.com'
# Windows
cmdkey /delete:git:https://github.com
# gh CLI, all platforms
gh auth logout && rm -f ~/.config/gh/hosts.yml
```

### Should the machine be rebuilt?

Sources disagree, so here is the rule this repository uses.

- **Persistence artifact found** (launch agent, systemd unit, Run key, scheduled
  task, git hook, shell profile, or an infected editor extension): **rebuild**.
  Something is configured to run again.
- **Repository artifacts only, no persistence, and you can account for what ran**:
  quarantine the artifacts, delete every local clone, rotate everything, and keep
  scanning weekly. A rebuild is still the safer choice if the machine holds
  production or financial access.

Either way, credential rotation is not optional. The stolen tokens work from
anywhere and do not care what you do to the laptop.

### What not to do

- Do not `git pull` into an existing clone of an infected repository. Delete the
  clone and re-clone after the remote is verified clean. A pull into an infected
  clone re-infects the remote.
- Do not open any repository from the affected set in an editor until the remote
  is clean and workspace trust is on.
- Do not rely on antivirus. The second stage is decrypted in memory and in most
  variants never lands on disk as an executable.
- Do not clean only the local side. The remote is where the malware propagates from.
- Do not clean only the remote. The implant on the machine re-pushes.
- Do not skip rotation because the file cleanup looked complete.

---

## 5. Hardening

### GitHub organization

| Control | Why it matters for this specific attack |
|---|---|
| **Require signed commits**, org-wide | The single highest-value control. The propagation script amends and backdates commits; signing makes that immediately visible |
| Block force pushes and restrict deletions on **every** repo, no exceptions | One whitelisted "unimportant" repo is how an org gets in through the side door |
| Require PR review, dismiss stale approvals | Nothing reaches a default branch unseen |
| CODEOWNERS on `.vscode/`, `*.config.*`, `package.json`, lockfiles, `.github/workflows/` | The exact paths this malware writes to |
| Secret scanning and push protection, including a historical scan | Finds secrets the malware may have committed |
| Short-lived fine-grained PATs only; OIDC federation for cloud CI | Removes the long-lived tokens the stealer is looking for |

### Developer machine

```jsonc
// VS Code user settings.json
{
  "task.allowAutomaticTasks": "off",           // kills the folderOpen vector outright
  "security.workspace.trust.enabled": true,
  "security.workspace.trust.startupPrompt": "always",
  "terminal.integrated.allowWorkspaceConfiguration": false,
  "extensions.autoUpdate": false,
  "extensions.autoCheckUpdates": false
}
```

```bash
npm config set ignore-scripts true    # then allow per project where a build needs it
```

Pin exact dependency versions, commit lockfiles, use `npm ci --ignore-scripts` in
CI, and proxy your registry if you can.

### Continuous scanning

Re-scan weekly for a month, then monthly. Reinfection with a rotated signature is
documented behaviour for this campaign, so a one-off scan is not enough. See
[`ci/`](ci/) for the workflow.

---

## 6. False positives you will see

These are expected. All four were confirmed harmless during a real cleanup.

| What you see | Why | What to do |
|---|---|---|
| Your own scan workflow flagged `INFECTED` | It contains the indicator strings because it searches for them | `triage-filter.sh` removes these. Add your own paths to its `BENIGN_RE` |
| `.vscode/settings.json` matched `folderOpen` | Only `.vscode/**tasks**.json` executes commands. `settings.json` is not the vector | Ignore. Verdict is `review`, never `INFECTED` |
| Every `.woff2` in a repo flagged as "not a font" | Git LFS stores a text pointer instead of the font, and a zero-byte placeholder has no magic bytes | Already handled: LFS pointers and empty files are skipped |
| A config file flagged for "content after module end" | Flat configs legitimately open with `export default [` on line 1 and run long | Already handled: the finding fires only when the remainder also looks like a payload |
| A README or incident writeup flagged `INFECTED` | Documenting the campaign means naming its indicators | Already handled: `.md` files are skipped. Use `--scan-docs` to include them |

If you find a new false positive, [open an issue](.github/ISSUE_TEMPLATE/false-positive.md)
with the file that triggered it. Precision matters more than reach here: a false
`INFECTED` in a tool people run during an incident costs everyone real time.

---

## 7. Sources

Primary, load-bearing:

- [PolinRider technical dossier — OpenSourceMalware](https://github.com/OpenSourceMalware/PolinRider) — indicators, YARA rule, scale figures
- [Socket live tracking page](https://socket.dev/supply-chain-attacks/polinrider) — current package counts
- [PolinRider: campaign expands across open source ecosystems — Socket](https://socket.dev/blog/polinrider-north-korea-linked-supply-chain-campaign-expands)
- [North Korean hackers publish 108 malicious packages — The Hacker News](https://thehackernews.com/2026/07/north-korean-hackers-publish-108.html)
- [Getting over PolinRider: a developer's guide — OpenSourceMalware](https://opensourcemalware.com/blog/developer-guide-getting-over-polinrider) — rotation checklist, C2 addresses, reinfection analysis

Additional reading:

- [PolinRider expands to the Packagist ecosystem — Developer Tech](https://www.developer-tech.com/news/polinrider-supply-chain-attack-expands-packagist-ecosystem/)
- [Surviving PolinRider: recovering GitHub repositories after a mass force-push — Karo Edaware](https://medium.com/@edawarekaro/surviving-polinrider-how-to-recover-your-github-repositories-after-a-mass-force-push-attack-ebe8175124a0)
- [Operation PolinRider: detection, containment and recovery — Karo Edaware](https://medium.com/@edawarekaro/operation-polinrider-the-complete-developers-guide-to-detection-containment-and-recovery-c3454bd660e3)
- [North Korean hackers target open source developers — SecurityWeek](https://www.securityweek.com/north-korean-hackers-target-open-source-developers-in-supply-chain-attacks/)

Indicators in [`ioc/`](ioc/) are drawn from the OpenSourceMalware dossier, Socket
and The Hacker News. Keep them current: see [`ioc/README.md`](ioc/README.md).

---

## Contributing

Issue and pull request templates are in [`.github/`](.github/). False positives
and missed detections are the two most useful things you can report. See
[CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

Security issues in these scripts go through the process in
[SECURITY.md](SECURITY.md), not a public issue.

## Disclaimer

These scripts read your repositories and your machine, and `restore.sh --apply`
moves branch pointers on GitHub. Read a script before you run it, run every step
as a dry run first, and keep the evidence directory until the incident is closed.
No warranty. See [LICENSE](LICENSE).
