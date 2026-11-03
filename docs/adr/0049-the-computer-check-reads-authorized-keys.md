# 0049. The computer check reads `~/.ssh/authorized_keys`

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-26 |

## Context

The `MicrosoftSystem64` RAT in the `toskypi` packages appends its own RSA key to
`~/.ssh/authorized_keys`, with the comment `bink@DESKTOP-N8JGD6T`
(MAL-2026-4345). A machine cleaned of the implant can still be entered over SSH.
Nothing read that file for content. The 2026-10-12 review could not carry the
comment as a string: `strong.txt` is matched inside project files, never in
`~/.ssh`.

## Decision

A list, `ioc/ssh-keys.txt`, of a key's comment or the base64 key itself. The
computer check reads `~/.ssh/authorized_keys` under the home directory being
checked, also with `--fs-only` on a disk, and splits each line into its fields:
options, key type, key, comment. A key whose comment or key equals an entry is
confirmed, as a new kind, `SshKey`, that is fixed by hand.

- **Matched on the field, never the line.** The options of a line can quote any
  string; a key that merely mentions the comment in an option is not the
  attacker's key. The same rule as the process check: match the specific field.
- **Never moved, never edited.** The file holds the owner's own keys. The
  finding names the line and says to remove it; step 4 of the guided flow adds
  "Remove the attacker's key from ~/.ssh/authorized_keys" and to read every
  other line.
- An unreadable file is a review line, not a pass. A missing one is clean.

## Consequences

- The attacker's known key is found, and the cleanup says to remove it.
- **A key with a new comment is not found.** The next build can use any
  comment and any key. This catches what is published; the step-4 advice to
  read every line is the part that does not depend on it.
- Only `~/.ssh/authorized_keys`. Windows' `administrators_authorized_keys`
  under `ProgramData`, and `authorized_keys2`, are not read.
- The folder check does not read it: it is a property of a machine.

## Related

- ADR-0029: host state through one boundary; this is a file under the home
  directory, so it needs none.
