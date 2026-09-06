# 0019. Restore is offered when possible, and explained when it is not

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

There are two remedies. Moving a branch back to its pre-attack commit is the
better one: no files are edited and the malicious commits simply stop being
reachable. Removing the payload is the fallback for when no earlier state
exists.

The guided menu only ever offered removal. When restore was possible it was not
on the menu, and when it was impossible nothing said why. Both look like the
same thing from the outside: a tool that only knows one trick.

## Decision

The menu is built from what the evidence supports.

When restore candidates exist, it offers fetching the pre-attack commits, then
restoring.

When none exist the option is still shown, greyed, with a dash instead of a
number and the reason attached:

```
    1  Read what was actually found
    -  Restore branches                 unavailable for these repositories
       GitHub keeps about 300 push events per repository for about 90 days.
       None survive here, so there is no recorded earlier state to move a
       branch back to. A limit of the GitHub API, not of this tool.
    2  Clean one repository, dry run first
```

An option that is simply absent and one that is impossible look identical from
the outside. Showing it in place says which this is.

Performing the restore stays outside the menu. It points at the verified targets
and the plan.

## Consequences

The operator can tell the difference between "this tool cannot do that" and
"that is not available for your incident", which is the difference between a
missing feature and a closed window.

**Restoring is still not automated from the menu.** Moving a branch pointer is
the one genuinely destructive operation here, and it stays behind its own
preflight and a person who has read the plan. That is deliberate, and it means
the better remedy is the one with more friction.

Menu numbering is dynamic, so no instruction anywhere may refer to an option by
its number. Disabled entries consume no number, or the caller's action list stops
lining up with what can be picked.
