# 0048. Add a review-level filename list

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-26 |

## Context

`filenames.txt` is `INFECTED` tier: a match means the payload is here. A name
the campaign uses that a legitimate project can also have cannot go in it.
`weak.txt` is review tier but matches file contents, not paths. So a name like
`spellright.dict` (a real VS Code extension's dictionary), a font named for a
weight, or `clr_init.vbs` had nowhere to go. The weekly reviews turned seven
such names away for that reason alone, from 2026-09-14 on.

## Decision

A second list, `ioc/filenames-review.txt`, in the same extended-regex syntax as
`filenames.txt` (`src/pattern.rs`), matched against every path the same way. A
match is a review line, never a finding of infection, and never on a path
`filenames.txt` already confirms. All three engines read it: the folder and
computer checks, the GitHub check (every path on every branch and tag, less
the operator's own detection tooling), and the CI scanner, which tolerates an
indicator set vendored before the file existed.

It starts with five patterns, each from a source a review read:
`spellright.dict`, `branch_structure.json`, `clr_init.vbs`, solid fonts named for
a weight Font Awesome does not ship (`fa-solid-[0-8][0-9]{2}.woff2`), and the
genuine `fa-solid-900.woff2` only under `public/fonts/`, where the campaign put
it. `config.bat` stays out: the 2026-09-28 review rejected it even at this
tier, since it would fire on any Windows project.

A pattern that does not parse stops the scan, as in `filenames.txt`.

## Consequences

- The reviews can carry these names instead of rejecting them, and the
  `.woff2` weights are covered by name as well as by the font check's magic
  bytes.
- **More review lines.** A project that really has a `spellright.dict` gets one.
  That is the tier's purpose; it never changes a clean result to infected.
- One more file the weekly routine has to classify against. The rule is the
  one `weak.txt` has: a match must be worth a person's look, and close calls
  stay out.
- A pattern error now names its file (`filenames.txt` or
  `filenames-review.txt`).

## Related

- ADR-0047: `weak.txt` in the folder and computer checks, the content-side
  counterpart.
