# 0005. Machine checks list recent changes only

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

Listing every editor extension and every launch agent on a developer machine produces hundreds of lines. A list nobody reads is worse than a shorter list somebody does.

## Decision

Editor extensions are listed if they changed in the last 60 days. Launch agents, daemons and scheduled tasks if they changed in the last 90. Both checks state their window and state what it excludes.

## Consequences

An extension compromised four months ago does not appear in the list.

This is a **listing** window, not a detection window: the contents of those files are matched against the indicators regardless of age. The distinction is now in the output, because it was not obvious.

The numbers are arbitrary. They are a guess at "recent enough to be suspicious, short enough to read".
