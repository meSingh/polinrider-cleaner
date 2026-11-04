---
title: "Removing PolinRider: do it in this order"
description: "Why PolinRider comes back after a cleanup, and the order that stops it: machines, credentials, repositories, then every future push."
---

If more than one of these applies to you, the order is fixed and it is not a
style preference.

1. **Check the machines.** Every machine that has touched the affected
   repositories, before you touch GitHub at all.
2. **Rotate every credential.** Assume everything reachable from the affected
   account is in someone else's hands.
3. **Get the payload out of the repositories.** Restore where push events
   survive, clean where they do not.
4. **Scan every future push**, so a reinfection is caught by CI rather than by
   a stranger.

:::danger
Cleaning a remote while an infected machine still holds a valid token puts you
back where you started within minutes. This is documented behaviour of the
campaign, and it is the single most common way a cleanup fails.
:::

## Why

Cleaning the remote while an infected machine still holds a valid token puts you
back where you started within minutes. The payload's whole purpose is to
propagate: it amends your latest commit and force-pushes the current branch to
`origin` with `-uf --no-verify`
([why your history looks clean](/threat/#why-your-git-history-looks-clean)). A
clean repository plus an infected laptop is a clean repository for about as long as it
takes the next commit to land.

This is documented behaviour of the campaign, and it is the single most common
way a cleanup fails.

## The guided flow does this for you

Running `polinrider` and answering `everything` walks the sequence: this
computer first, then GitHub. Rotating credentials is yours to do between the
two. The per-track guides are next, and
[Remove PolinRider: the full cleanup checklist](/docs/guides/getting-rid-of-polinrider/)
puts every step on one page, with the checks still done by hand and when to
rebuild. What has moved since OpenSourceMalware's August cleanup guide is in
[Cleaning up PolinRider: what has changed since August](/updates/2026-11-04-developers-guide-getting-rid-of-polinrider/).
