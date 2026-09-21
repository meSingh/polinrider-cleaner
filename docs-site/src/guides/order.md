# Order matters

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

## Why

Cleaning the remote while an infected machine still holds a valid token puts you
back where you started within minutes. The payload's whole purpose is to
propagate: it force-pushes to every remote the machine can write to. A clean
repository plus an infected laptop is a clean repository for about as long as it
takes the next commit to land.

This is documented behaviour of the campaign, and it is the single most common
way a cleanup fails.

## The guided flow does this for you

Running `./polinrider.sh` with no arguments walks the sequence, and will not
offer to clean a remote before the machine it is running on has come back clear.
If you would rather drive it yourself, the per-track guides are next.
