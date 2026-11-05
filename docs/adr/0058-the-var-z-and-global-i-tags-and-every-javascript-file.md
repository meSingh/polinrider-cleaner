# 0058. Confirm the var z and global.i tags, and read every JavaScript file

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-11-05 |

## Context

A study of the files at the paths OpenSourceMalware's PolinRider lists name
(July and April 2026 lists) found 310 still-infected files whose payload no
indicator in 3.0.0 matched. Run through the 3.0.0 binary in the sandbox:

- the machine check confirmed none, gave 281 a review line (a line over 4,000
  characters, or a weak signal) and called 29 clean;
- the GitHub check, on 12 samples against a pretend GitHub, confirmed none,
  gave 5 a review line and said nothing about 7.

The files fall into four shapes, all on the last line, behind padding:

| Shape | Files | Padding | Opens with |
|---|---|---|---|
| Hex blob | 241 | 149 spaces | the `var z` tag, `var z = '-N-N';` |
| `var NX` table | 24 | 266 tabs | the same `var z` tag |
| `global.i` tag | 37 | 507 to 2,236 spaces | `global.i` set to A10 or A11 |
| One obfuscator build | 8 | 997 spaces | function names from that build |

Two more gaps sat beside these:

1. **The machine check read build configs only.** The same payload, behind the
   same padding, appended to `jest.config.js`, `truffle-config.js`, a route
   file or `App.jsx` gave exit 0 or 1. The cleanup PR trial of 2026-11-05 found
   4 of 24 infected repositories in this state. The GitHub check confirmed all
   four, because `git grep` reads every file, so the two checks disagreed.
2. **A pattern match could not be cut.** ADR-0052 refused to strip a file
   matched only by a content pattern, because a pattern gave no position to
   cut from. In the GitHub `remove` fix a file that cannot be stripped is
   deleted whole. A `var z` pattern on its own would have made `remove`
   delete the project's `postcss.config.mjs` instead of cleaning it.

**Sources.** No publication states the `var z` string or the A10 and A11
values. They are read from the infected files that OpenSourceMalware's lists
name, at those paths. SafeDep published the `global.i` shape in its hunt
query ("PolinRider Switches to Ethereum C2 in 30+ Repositories", September
2026), and `strong.txt` already pins its A8 value. Mandeep accepted
indicators derived from published samples on 2026-11-05.

## Decision

1. **Two patterns in `ioc/strong-patterns.txt`, at the confirmed tier:**
   - the `var z` tag behind at least 64 spaces or tabs. The padding is part of
     the pattern, so minified code that says `var z = '-1'` does not match. The
     bracket holds a real tab character, because `\t` in a bracket is a
     backslash and a `t` to both `src/pattern.rs` and `grep -E`;
   - the `global.i` tag set to `A` and one or two digits, followed by a dash or
     the closing quote, in either quote style, with or without spaces. It takes
     in the pinned A8 forms. `weak.txt` keeps the looser `global.i = 'A` at
     review.

   Each carries a source note naming the lists and the study. A derived
   indicator is acceptable where the files it is read from are on a published
   list and the note says so.
2. **The machine check reads every `.js`, `.mjs`, `.cjs`, `.ts`, `.jsx` and
   `.tsx` file for a confirmed indicator,** not only the build configs. It
   reads them in the walk the other checks use, so `node_modules` and the
   other pruned folders stay unread. Detection tooling is set aside by the
   GitHub check's own path rules, judged from the scanned folder. The review
   rules (a line over 4,000 characters, a payload-shaped tail) stay with the
   build configs: a long line is ordinary in a bundle. A hit reads "source file
   contains an indicator", and `clean` strips it under the same rules as a
   config, keeping the original in quarantine. A source file is never moved
   whole.
3. **A pattern match is placed at its fixed part and cut like a fixed
   string.** `Indicators::first_at` returns the offset of the first fixed part
   whose window matches, and strip cuts at the padding before it, under the
   same refusals: padding, a module end above, nothing of the project's
   after, balanced brackets, no indicator left. `Refusal::PatternOnly` remains
   for a file that is not valid UTF-8, where a place in the text is not a
   place in the bytes. This replaces ADR-0052's rule that a pattern match is
   never stripped.

**Not in this decision:** padded code as a review finding in every
JavaScript file, in both checks. It would catch the fourth shape, the one
obfuscator build, by its shape, and give the GitHub check the long-line rule
it lacks. It waits for 3.1.

## Consequences

- **On the gap set, 302 of 310 are confirmed** by the machine check, against
  none in 3.0.0. The 8 files of the one obfuscator build stay at review. None
  is clean, against 29 before. On the 12 GitHub samples, 10 are confirmed and
  2 (the obfuscator build) are review, against 0 confirmed, 5 review and 7
  silent.
- **The two checks agree** about a payload in a JavaScript file that is not a
  build config.
- **`remove` keeps the file.** A config or source file matched by a pattern is
  cut at the padding, and the corpus checks the result byte for byte.
- **A derived indicator is only as good as our reading of the samples.** If
  a legitimate project writes the `var z` tag behind 64 columns of padding,
  or sets `global.i` to A and two digits, every user gets a false INFECTED.
  Measured: none of 30,607 `.js`, `.mjs`, `.cjs` and `.ts` files from the
  latest releases of 41 popular npm packages is confirmed by the 3.0.1
  binary. That corpus is published packages, not years of mixed code in a
  home directory.
- **The machine check reads more.** Every JavaScript file under the scanned
  folders is read once more for strong indicators. The `weak.txt` check
  already read every file under 1 MiB. A full check of the npm corpus took
  25 seconds in the sandbox; there is no 3.0.0 time to compare it with.
- **Detection code written in JavaScript can now be confirmed locally,**
  as the GitHub check already confirmed it, unless it sits where the
  detection-tooling rules set it aside (`docs/`, `.github/polinrider/` and
  the rest of that list).
- **A source-file hit is counted as a config in the summary.** The finding
  line says "source file", but the plain-words summary still counts it
  under "config file with the payload hidden in it". A separate kind is a
  larger change than a patch release should carry.
- **A cut is placed at a fixed part, not at the start of the match.** Where
  two fixed parts sit within one window and the first is not behind padding,
  the cut is refused rather than made. That is the safe direction.
- **The tab in the bracket is invisible.** An editor that turns tabs into
  spaces would lose the `var NX` shape without a word. A unit test checks the
  shipped pattern against tab padding.

## Related

- ADR-0052: content patterns at the confirmed tier. Its rule that a pattern
  match is not stripped is replaced here.
- ADR-0031: the strip, and the refusals this keeps.
- ADR-0025: the pruned walk the source-file read reuses.
- ADR-0047: the folder and computer checks read `weak.txt`, the precedent for
  reading every file.
