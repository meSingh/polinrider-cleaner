---
title: Is my GitHub repository infected with PolinRider?
description: Five checks that tell you whether PolinRider reached a GitHub repository, without running its code, and why the commit history alone cannot tell you.
---

Perhaps someone filed an issue saying your repository carries PolinRider.
Perhaps your name is on a researcher's list, or you have only read about
the campaign and want to know. You can find out without running anything
from the repository.

## Why the history will not tell you

PolinRider is a supply-chain campaign that security researchers link to
North Korea. It spreads from infected computers: it adds its loader to a
project, amends the last commit with the original date and author kept and
force-pushes it. So the commit list on GitHub and `git log` look exactly as
they did. GitHub's own record of each push cannot be faked that way, and
that is where the fifth check looks.

## Five checks

Run the first four on a fresh clone and do not open it in VS Code, install
it or build it. A clone on its own runs nothing.

**1. The end of the config files.** The loader is appended to a config file
that a build loads, after a long run of spaces. This lists any config line
over 4,000 characters:

```bash
find . -name '*.config.*' -not -path '*/node_modules/*' \
  -exec awk 'length > 4000 { print FILENAME ": line " FNR ", " length " characters" }' {} +
```

Code after the config's `export default` is the loader.
[Code after export default](/docs/found/code-after-export-default/) explains
what you are looking at.

**2. A task that runs on folder open.** Look in `.vscode/tasks.json` for
`"runOn": "folderOpen"` with a command that runs `node` on a font file or
downloads and runs something. See
[a tasks.json that runs on folder open](/docs/found/tasks-json-folder-open/).

**3. Fonts that are not fonts.**

```bash
find . -path ./node_modules -prune -o -path '*/fonts/*' -type f -exec file {} +
```

Every real font shows as a font format. One that shows as text or JavaScript
is the payload, whatever it is called.

**4. The .gitignore.** If `.gitignore` lists `temp_auto_push.bat`,
`temp_interactive_push.bat`, `config.bat` and `branch_structure.json`, at
least one computer that pushes to this repository is infected. That holds
even if no payload is in the repository yet. See
[Found temp_auto_push.bat?](/analysis/temp-auto-push-bat-polinrider/)

**5. Force-pushes nobody made on purpose.** Open the repository's activity
page, `https://github.com/OWNER/REPO/activity`, and look for force-pushes.
Each one shows the account whose credentials pushed and GitHub's own time,
neither of which the malware can change. A force-push that nobody on the
team remembers is the infection.

Check every branch, not only the default one. The campaign pushes to
whichever branch was checked out on the infected computer.

## If any check finds something

1. **Stop pushing from every computer that pushes to the repository** until
   each one is checked. The repository is a symptom; the infected computer
   is the cause.
2. **From a computer you trust,** block force-pushes, require signed commits
   and pause automatic deploys. Revoke any GitHub or OAuth app you do not
   recognise, and warn your collaborators and anyone who installs your
   package.
3. **Check and clean the computers, then change every password and token
   from a clean one,** and only then clean the repository. Cleaned in the
   other order, it comes back.

The [developer's guide](/guides/developers-guide-getting-rid-of-polinrider/)
covers the whole cleanup, and the
[checklist](/docs/guides/getting-rid-of-polinrider/) puts every step on one
page.

## Checking with polinrider

polinrider is a free, open source scanner and cleanup tool for this
campaign, made by the maintainer of this site. Run it and answer `account`
or `organization`, and it checks every branch and tag of every repository
and reads GitHub's push record for each. Where that record still holds a
clean commit, it offers to restore the branch to it. It changes nothing
until you type `yes`. Start with the [quick start](/docs/quick-start/), and
see [checking a GitHub account](/docs/guides/account/) for what it does
step by step.

## Sources

- OpenSourceMalware, [Hundreds of GitHub Repos Compromised By DPRK's PolinRider Campaign](https://opensourcemalware.com/blog/polinrider-dprk-compromised-hundreds-of-github-repos), 8 March 2026
- OpenSourceMalware, [PolinRider is A/B Testing its Way Past Your Detections](https://opensourcemalware.com/blog/polinrider-is-a-b-testing-its-way-past-your-detections), 26 September 2026
- OpenSourceMalware, [How to Investigate a GitHub Repo's Real History](https://opensourcemalware.com/blog/how-to-investigate-a-github-repos-real-history), 6 October 2026
- [Why your Git history looks clean](/what-is-polinrider/#why-your-git-history-looks-clean)
