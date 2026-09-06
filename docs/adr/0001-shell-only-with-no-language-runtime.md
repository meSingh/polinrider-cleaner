# 0001. Shell only, with no language runtime

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

This tool is run by people whose machines may already be compromised, often in a hurry, and frequently on a machine that is not their usual one. Every dependency is something to install first and something more to trust.

Node and Python are both plausible choices and both are excluded by the same reasoning: the campaign this tool cleans up ships malicious npm packages, so requiring an npm install to clean an npm compromise is circular.

## Decision

Everything is POSIX-ish shell with `git`, `gh` and `jq`. No language runtime, no package manager, no build step. Bash 3.2 is the floor, because that is what ships with macOS.

`git filter-repo` is the single exception, and only ever as an optimisation: see [ADR-0013](./0013-history-rewriting-uses-filter-repo-when-it-is-installed.md).

## Consequences

Some things are slower and more verbose than they would be in a real language. Array handling, JSON parsing and string manipulation are all more awkward.

In exchange the tool can be read end to end by anyone who knows shell, runs on a machine with nothing installed, and cannot itself pull in a compromised dependency.

Bash 3.2 rules out associative arrays, `mapfile`, and `${var^^}`. That constraint is easy to forget and CI does not catch it, because the runners have Bash 5.
