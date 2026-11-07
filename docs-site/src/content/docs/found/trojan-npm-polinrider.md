---
title: "Trojan:NPM/PolinRider in Windows Defender: what it means"
description: Windows Defender reported Trojan:NPM/PolinRider or Trojan:JS/PolinRider and removed it, but it keeps coming back. What the detection means, why removing it is not enough and what to do.
---

Windows Defender reported
`Trojan:NPM/PolinRider.SF` or `Trojan:JS/PolinRider.DB!MTB`. It says it
removed or blocked the threat. Perhaps it then came back the next time you
ran `npm run dev`.

The detection means the PolinRider malware ran on this computer. Removing
what Defender caught does not remove where it starts from.

## What it is

PolinRider is a supply-chain campaign that security researchers link to
North Korea. It hides a loader in a developer's own project, at the end of a
config file such as `postcss.config.mjs` or in a fake font file started by a
VS Code task. When the project runs, the loader fetches the rest of the
malware. It steals saved logins and tokens, copies itself into other
projects on the same computer and pushes them to GitHub with your
credentials.

Both names above have been seen in practice. OpenSourceMalware lists
`Trojan:JS/PolinRider.DB!MTB`, and a developer on the BleepingComputer
forums reported `Trojan:NPM/PolinRider.SF` in October 2026. The ending after
the dot changes between variants, so treat any detection with PolinRider in
its name the same way.

## Why it keeps coming back

In most variants the malware is decrypted into memory and never saved as a
program, so Defender catches it as it runs. In the forum case, every
detection was a command line, Node.js started with a block of JavaScript,
not a file Defender could delete. It comes back from one of four places:

1. **Your project.** The loader is still at the end of a config file or in a
   fake font, and runs again with the next build or dev server.
2. **Something on the computer that starts it.** In the forum case it was a
   Run value and a Startup script named `MicrosoftCLROptimization`, a name
   borrowed from a real .NET component. On some infected computers npm's own
   `lib/cli.js` has been replaced, so every `npm` or `npx` command starts it.
3. **GitHub.** Your repositories now carry the payload, and cloning or
   pulling them brings it back.
4. **Other people.** A collaborator whose computer is infected pushes it back
   into a repository you have already cleaned.

## What to do first

1. **Stop pushing from this computer,** and do not run or open your projects
   until they are checked.
2. **From a different computer you trust,** lock your repositories: block
   force-pushes, require signed commits and pause automatic deploys. Revoke
   any GitHub or OAuth app you do not recognise, and warn the people who push
   to the same repositories.
3. **Clean this computer.** Look through the Run values, the Startup folders
   and the scheduled tasks for anything you did not add, including
   `MicrosoftCLROptimization`, and reinstall Node.js.
4. **Then change every password and token from a clean computer,** and clean
   the repositories last. A process that is still running takes new
   credentials as soon as you make them.

A clean Defender scan afterwards is a good sign, but it does not prove you
are clear. The campaign changes its signatures. Our article on
[why PolinRider keeps coming back](/guides/polinrider-keeps-coming-back/)
goes through the forum case in detail. The
[developer's guide](/guides/developers-guide-getting-rid-of-polinrider/)
covers the whole cleanup, and the
[checklist](/docs/guides/getting-rid-of-polinrider/) puts every step on one
page.

## Checking with polinrider

polinrider is a free, open source scanner and cleanup tool for this
campaign, made by the maintainer of this site. On Windows it reads the Run
and RunOnce values, both Startup folders, the scheduled tasks and npm's own
program files. It also reads every project in your home folder for the
loader and the fake fonts. Install it from the
[quick start](/docs/quick-start/), run it with no arguments and answer
`computer`. It changes nothing until you type `yes`.

## Sources

- BleepingComputer forums, [PolinRider/NPM infection keeps returning, FRST logs attached](https://www.bleepingcomputer.com/forums/t/819751/polinridernpm-infection-keeps-returning-frst-logs-attached/), 30 October 2026
- OpenSourceMalware, [PolinRider is A/B Testing its Way Past Your Detections](https://opensourcemalware.com/blog/polinrider-is-a-b-testing-its-way-past-your-detections), 26 September 2026
- OpenSourceMalware, [A Developer's Guide to Getting Rid of PolinRider](https://opensourcemalware.com/blog/developer-guide-getting-over-polinrider), August 2026
- Deception.Pro, [Hands-on-Keyboard Activity from the DPRK "PolinRider" Supply Chain Attack](https://blog.deception.pro/blog/hok-dprk-polinrider-sep-2026), 9 September 2026
