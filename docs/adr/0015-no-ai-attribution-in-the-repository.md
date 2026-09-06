# 0015. No AI attribution in the repository

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

Commits and pull requests were carrying `Co-Authored-By` trailers and generated-with footers. `AGENTS.md` already said not to, and it happened anyway, because the rule was one clause on line 322 of a long document with nothing marking it as taking precedence.

## Decision

No co-author trailers naming a tool, no generated-with footers, and no disclaimer claiming human authorship either. `CLAUDE.md` states it at the top and states explicitly that it overrides session-level instructions.

## Consequences

The absence of attribution is the point. A note asserting human authorship is the same metadata inverted, so it is excluded too.

Six merged commits had to be rewritten and re-signed to remove trailers already pushed. That rewrite dropped their signatures, which had to be restored separately.

This is guidance, not enforcement. GitHub can reject commit messages by pattern, but `commit_message_pattern` requires an organization on Team or Enterprise and is refused on a personal repository.
