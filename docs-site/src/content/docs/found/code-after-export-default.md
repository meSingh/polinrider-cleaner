---
title: Strange code after export default in postcss.config.mjs
description: A long line of scrambled JavaScript after the end of postcss.config.mjs, tailwind.config.js or eslint.config.mjs is the PolinRider malware. What it is and what to do first.
---

You opened `postcss.config.mjs`, `tailwind.config.js`, `eslint.config.mjs` or
`next.config.mjs`. The config looks normal and ends where it should, with
`export default config;` or a closing bracket. Then either there is a huge
gap, or the last line goes on far to the right, and somewhere past it is a
block of scrambled JavaScript you did not write.

That is the PolinRider malware. Treat the computer that has this project on
it as infected until you have checked it.

## What it is

PolinRider is a supply-chain campaign that security researchers link to
North Korea. Its loader is appended to the end of a config file that a build
tool imports, after a long run of spaces so that it sits off the edge of the
screen. Whenever the file loads, the loader runs: the next `npm run dev`, the
next build, the next CI job.

OpenSourceMalware, which first reported the campaign in March 2026, found
`postcss.config.mjs` in about six in ten of the infected repositories it
listed. Of the 2,417 repositories in its July list, 1,362 were hit in
`postcss.config.mjs`, 325 in `tailwind.config.js` and 224 in
`eslint.config.mjs`. Any file a build loads can carry it, including `App.js`
and `vite.config.js`.

The loader fetches the rest of the malware at run time. It steals saved
logins and tokens and copies itself into other projects on the same
computer. Then it pushes them to GitHub with your own credentials, keeping
each commit's original date, so `git log` shows nothing new.

## Check without running it

Do not run the project, its tests or any `npm` script, and do not open the
folder in VS Code. Reading a file in a terminal is safe. This lists every
config line longer than 4,000 characters, which is where the loader sits:

```bash
find . -name '*.config.*' -not -path '*/node_modules/*' \
  -exec awk 'length > 4000 { print FILENAME ": line " FNR ", " length " characters" }' {} +
```

A real config is rarely that long. If a line shows up and the file has more
code after its `export default`, it is the loader. A flat ESLint config that
opens with `export default [` and runs long is normal: what matters is code
after the config ends.

Look at `.vscode/tasks.json` in the same project as well, and at any `.woff2`
file under `public/fonts/`. The campaign also uses
[a task that runs when the folder opens](/docs/found/tasks-json-folder-open/)
and a [font file that is really JavaScript](/analysis/fake-woff2-font-javascript/).

## What to do first

1. **Stop pushing from this computer.** Every push can carry the payload to
   another repository.
2. **Do not delete the lines and carry on.** If the project ran on this
   computer, the malware has already run, and it can write itself back into
   the file and into your other projects. Cleaning the file first and the
   computer later does not last.
3. **From a different computer you trust,** lock the repository: block
   force-pushes, require signed commits and pause automatic deploys. Revoke
   any GitHub or OAuth app you do not recognise, and warn the people who push
   to the same repository.
4. **Check the computer, then change every password and token from a clean
   one,** then clean the repositories. The order matters, because a process
   that is still running takes new credentials as soon as you make them.

The [developer's guide](/guides/developers-guide-getting-rid-of-polinrider/)
walks through the whole cleanup, and the
[checklist](/docs/guides/getting-rid-of-polinrider/) puts every step on one
page.

## Checking with polinrider

polinrider is a free, open source scanner and cleanup tool for this
campaign, made by the maintainer of this site. It reads the config files
without running them and reports a loader it recognises as confirmed.
Anything that only looks like one, such as code after the end of a config,
is listed for you to review. Install it from the
[quick start](/docs/quick-start/), run it with no arguments and answer
`folder` for one project, `computer` for this computer or `account` for
every branch and tag on your GitHub account. It changes nothing until you
type `yes`.

## Sources

- OpenSourceMalware, [Hundreds of GitHub Repos Compromised By DPRK's PolinRider Campaign](https://opensourcemalware.com/blog/polinrider-dprk-compromised-hundreds-of-github-repos), 8 March 2026
- OpenSourceMalware, [PolinRider dossier](https://github.com/OpenSourceMalware/PolinRider), including `polinrider-compromised-repos-july10.csv`, from which the file counts above are taken
- OpenSourceMalware, [A Developer's Guide to Getting Rid of PolinRider](https://opensourcemalware.com/blog/developer-guide-getting-over-polinrider), August 2026
- [False positives you will see](/docs/reference/false-positives/), for config files that run long for innocent reasons
