# 0002. Exit code 3 means the scan could not run

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

The workspace scanner originally used `exit 2` for both "confirmed finding" and "could not run", and the entry point treated anything at or above 2 as a finding.

Pointing the tool at a directory that did not exist therefore printed the full compromise playbook: *"Something confirmed. This folder contains a confirmed indicator."* The same conflation covered a missing `git`, a missing `jq`, an unauthenticated `gh`, and a GitHub scan that failed halfway.

## Decision

Four exit codes, used consistently by every script:

| Code | Means |
|---|---|
| 0 | nothing found |
| 1 | review items only |
| 2 | something confirmed |
| 3 | the scan could not run |

`3` never enters the worst-result tracking, so it can never select a playbook. It also suppresses the "nothing confirmed" result, because an incomplete run is not a clean one.

## Consequences

A tool that says "confirmed" when it did not look is worse than one that says nothing, and a tool that says "clean" when half its checks failed is worse still. Both are now impossible.

Callers that treated non-zero as "infected" need to distinguish 3. CI treats any non-zero as failure, which is correct there.

`ci/selftest-entrypoint.sh` asserts the whole contract, including that 0 and 2 still mean what they did.
