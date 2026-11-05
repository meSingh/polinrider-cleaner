---
title: A .vscode/tasks.json that runs when you open the folder
description: A tasks.json with runOn folderOpen that runs node on a font file, or downloads and runs a script, is how PolinRider starts in VS Code. How to tell it from a real task and what to do.
---

You found a `.vscode/tasks.json` in a project, perhaps one handed to you as
a job-interview coding test or one that came in a stranger's pull request. One of its tasks has
`"runOn": "folderOpen"`, and its command runs `node` on a file in
`public/fonts/`, or downloads something with `curl` or `wget` and runs it.

That is how the PolinRider malware starts. The task runs as soon as you open
the folder in VS Code and trust it. You don't need to build or run anything.

## What it is

VS Code can run a task automatically when a folder opens. Teams use it for
real work, such as starting a watcher. The PolinRider campaign, which
security researchers link to North Korea, uses it to start its loader.
OpenSourceMalware first described the trick in January 2026, in fake
job-interview repositories, and in March tied it to PolinRider under the
name TasksJacker. The task is usually labelled `eslint-check`, runs with its
output hidden and points `node` at a file named like a font, such as
`public/fonts/fa-solid-400.woff2`, which is really JavaScript.

The campaign moves things around to dodge detection. OpenSourceMalware has
seen the font renamed from `fa-solid-400.woff2` to `-500`, `-900` and
`fa-solid-300.llf`, which is not a font extension at all. A file name proves
nothing either way. Read the command.

## Real task or malware

A folder-open task is malware if its command does either of these:

- runs `node` on a font file, or on any file under `public/fonts/`;
- downloads something and runs it in the same line, for example `curl ... | sh`
  or `wget ... && node ...`.

A folder-open task that starts your own dev server or a file watcher, with a
command your team wrote, is normal. To check whether a font is real without
running it:

```bash
file public/fonts/*
```

A real font shows as `Web Open Font Format`. One that shows as text or
JavaScript is the payload.

## What to do first

1. **Close the folder and do not trust it.** If VS Code asks whether you trust
   the authors, choose no. In VS Code's settings, set
   `task.allowAutomaticTasks` to `off` so that no folder can run a task by
   itself.
2. **If you already opened and trusted it, assume the computer is
   infected.** Stop pushing from it. The loader steals saved logins and
   tokens, and copies itself into your other projects. Then it pushes them to
   GitHub with your credentials, keeping each commit's original date.
3. **From a different computer you trust,** lock the repositories you can
   push to: block force-pushes, require signed commits and pause automatic
   deploys. Revoke any GitHub or OAuth app you do not recognise, and warn the
   people who push to the same repositories.
4. **Check the computer, then change every password and token from a clean
   one,** then clean the repositories.

If you only cloned the project and never opened it in an editor or ran it,
the task has not run. Delete the clone and tell whoever sent it to you.

The [developer's guide](/guides/developers-guide-getting-rid-of-polinrider/)
covers the whole cleanup, and the
[checklist](/docs/guides/getting-rid-of-polinrider/) has every step on one
page.

## Checking with polinrider

polinrider is a free, open source scanner and cleanup tool for this
campaign, made by the maintainer of this site. It reports a folder-open task
that carries a known PolinRider command, or a fake font, as confirmed. Any
other folder-open task is listed for you to read, because teams use them
for real work. Install it from the [quick start](/docs/quick-start/), run
it with no arguments and answer `computer` to check every project on this
computer at once. It changes nothing until you type `yes`.

## Sources

- OpenSourceMalware, [DPRK Contagious Interview "Fake Font" Abuses VS Code Tasks](https://opensourcemalware.com/blog/contagious-code-fake-font), 28 January 2026
- OpenSourceMalware, [TasksJacker DPRK Attack Compromises GitHub Users Via VS Code Tasks](https://opensourcemalware.com/blog/tasksjacker-dprk-attack-github-vscode), 31 March 2026
- OpenSourceMalware, [PolinRider DPRK Attack Expands Across GitHub](https://opensourcemalware.com/blog/polinrider-rides-again-north-korean-attack-expands-across-github), 12 April 2026
- OpenSourceMalware, [PolinRider is A/B Testing its Way Past Your Detections](https://opensourcemalware.com/blog/polinrider-is-a-b-testing-its-way-past-your-detections), 26 September 2026
- Our analysis of the [fake font file](/analysis/fake-woff2-font-javascript/)
- [False positives you will see](/docs/reference/false-positives/), for folder-open tasks that are harmless
