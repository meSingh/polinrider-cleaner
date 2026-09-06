# 0006. Scan roots are confirmed, not guessed

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

Earlier versions guessed at `~/Sites`, `~/Projects`, `~/code` and similar, silently.

The first real run of this tool scanned `~/Sites` and `~/Documents`, and never looked at `~/.claude/plugins`, which holds code that runs at every session start. Nothing in the output said so.

## Decision

Candidate directories are detected and shown, in two groups: code directories, and directories holding code that runs by itself such as editor extensions and agent plugins. The operator confirms, extends, replaces, or asks for the whole home directory.

`--roots` still takes the list directly, and `--yes` uses the detected set, so scripts are unaffected.

## Consequences

One more question before a scan starts, which is the point.

The second group exists because plugin and extension directories execute on their own schedule and are easy to forget. They are more interesting to an attacker than a documents folder.

Root lists are carried as arrays throughout. They were space-separated strings, which silently split paths like `~/Library/Application Support/Code/User` in half.
