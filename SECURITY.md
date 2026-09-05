# Security policy

## Reporting a vulnerability in these scripts

If you find a way to make a script in this repository destroy data, restore a
branch to an attacker-controlled commit, leak a token, or execute attacker-
controlled input, report it privately through GitHub's **Report a vulnerability**
button on the Security tab. Do not open a public issue.

Include the script, the exact command, and what an attacker gains.

## What is in scope

- Any path where a script deletes data, or changes state without `--apply`
- A restore that can be steered onto a malicious commit
- Token or credential exposure: in output, in a report file, in a clone URL, or
  in a process argument list
- Command injection through a repository name, branch name, file path or
  indicator file

## What is not in scope

- False positives and missed detections. Those are normal issues, so use the
  [issue templates](.github/ISSUE_TEMPLATE).
- The PolinRider campaign itself. Report campaign findings to
  [Socket](https://socket.dev/supply-chain-attacks/polinrider) or the
  [OpenSourceMalware project](https://github.com/OpenSourceMalware/PolinRider).

## Trusting this repository before you run it

You are being asked to run shell scripts during a security incident, which is
exactly the situation where you should be suspicious.

- Every script is plain shell or PowerShell. Read it. The longest is just over 400 lines.
- Nothing fetches code at runtime. The only network calls are `git clone`,
  `gh api`, and the checkout action in CI.
- Everything except `restore.sh --apply` and the local checks with `--apply` is
  read-only.
- Pin what you run: clone at a commit SHA you have read, not a moving branch.
