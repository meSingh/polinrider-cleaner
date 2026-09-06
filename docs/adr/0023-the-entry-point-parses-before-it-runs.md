# 0023. The entry point parses before it runs

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

Bash reads a script in chunks as it executes. A script edited while it is
running executes part of one version and part of another, and if the edit lands
mid-statement it dies with a parse error far from anything the operator did:

```
./polinrider.sh: line 800: unexpected EOF while looking for matching `"'
./polinrider.sh: line 801: syntax error: unexpected end of file
```

This tool is interactive and can sit at a prompt for minutes. A `git pull` in
another window, or an update mid-incident, is enough to trigger it.

## Decision

The body of `polinrider.sh` is wrapped in a `{ ... }` group. A compound command
has to be parsed to its closing brace before any of it runs, so the whole file
is read up front.

## Consequences

Demonstrated rather than assumed: an ungrouped script edited 0.4 seconds into a
2 second run loses everything after the edit point; the grouped one completes.

A brace group is not a subshell, so variables, functions, traps and `exit` all
behave exactly as before. The cost is two lines and one level of indentation
that is not applied, since re-indenting the file would obscure the diff.

Only the entry point needs this. The engines are short and non-interactive, so
the window in which an edit could land is negligible.
