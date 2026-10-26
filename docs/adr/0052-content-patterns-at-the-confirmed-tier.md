# 0052. Match campaign patterns in file contents

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-26 |

## Context

`strong.txt` is fixed strings. Some indicators rotate a part: the fake-font
task runs `node ./public/fonts/fa-solid-400.woff2`, then 500, then 900, in
other folders. `strong.txt` carries the 400 form; the live 500 and 900 forms
went unmatched by content, caught only by the font check's magic bytes, and
the weekly reviews have asked for a regex content tier since 2026-09-28.
`src/pattern.rs` already parses the regex subset `filenames.txt` uses, but it
backtracks: fine for a path, a risk on a payload line hundreds of kilobytes
long.

## Decision

`ioc/strong-patterns.txt`, extended regex at the `INFECTED` tier, matched
wherever `strong.txt` is: this computer, folders, the GitHub check and the CI
scanner, decoded from escapes where strong strings are.

Every pattern must have a fixed part of at least three characters, one
alternative at the top, and no `^` or `$`; anything else stops the scan with
exit 3, as an unparseable filename pattern does. A pattern is tried only in a
window of 512 characters either side of each place its fixed part occurs, at
most a thousand places a file. The GitHub check finds candidate files with
`git grep -F` on the fixed parts and tries the pattern on each. The CI
scanner hands the file to `grep -E`.

A config matched only by a pattern is confirmed and not stripped: a pattern
gives no position to cut from, the same as an escape-coded payload.

The first pattern is `node +[^ "']*fa-solid-[0-9]{3}\.woff2`: node followed
directly by a font path, so a script that is only handed a font name does not
match.

## Consequences

- A rotating indicator is one line instead of one per variant, and the reviews
  can carry one.
- **A pattern is a sharper tool than a string.** It is `INFECTED` tier, so a
  pattern that matches something legitimate costs every user a false alarm.
  Close calls stay out, as for `strong.txt`.
- **A match more than 512 characters from the fixed part is not seen**, and
  neither is a pattern that would need to span that far. The window is the
  price of a matcher that backtracks.
- `grep -E` in the CI scanner and `src/pattern.rs` must agree. The subset is
  small and parse errors stop the binary, but the two can differ at edges.

## Related

- ADR-0040: escape codes, decoded for this tier too.
- ADR-0048: the review-level filename list, the path-side counterpart.
