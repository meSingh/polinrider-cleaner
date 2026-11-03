# 0050. A `node` process running a script from the implant's folder is the implant

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-26 |

## Context

The ULID-cluster packages copy `payload.js` into the `MicrosoftSystem64`
directory and run it as `node payload.js --agent`, not as a native binary
(2026-10-12 review). `implant-names.txt` is compared with a process's name
only, by design, so this process, named `node`, was never seen. The files on
disk are caught by the `implant-paths.txt` directories; an agent still running
after its folder is deleted was not.

The process name is compared, and never the command line, because matching a
full command line reports anyone who mentions the implant: an administrator
grepping for it, an editor with the indicator file open, this scanner.

## Decision

For a process whose name is `node` or `node.exe`, and only for one, read one
field of its command line: the script it runs. That is the first argument that
is not a flag, through the first one ending in `.js`, `.cjs` or `.mjs`, so a
path with a space in it is read whole; a quoted program path on Windows is
skipped as one. If any folder in that path is named, ignoring case, as an
entry in `implant-names.txt`, the process is the implant, reported and killed
like the native one.

## Consequences

- The running JavaScript agent is found, on Linux, macOS and Windows.
- **Only `node`.** The same script run by `bun`, `deno` or a renamed copy of
  node is not seen. The folder on disk still is, while it exists.
- **A flag that mentions the folder is not the script.** `node server.js --log
  /tmp/MicrosoftSystem64` is not reported, and a corpus case pins it.
- Reading an argument is a step past "name only". It is one field, parsed by
  position, of one program, and the match is on a whole folder name, so the
  failure the rule exists for (reporting whoever mentions the implant) does not
  return.

## Related

- AGENTS.md rule 5: match the specific field, never the whole line.
