---
title: The indicator set
---

One source of truth, in [`ioc/`](https://github.com/meSingh/polinrider-cleaner/tree/main/ioc).
Every script reads these files at runtime, so adding an indicator there updates
the organization scanner, the account scanner and all three local checks at once.

| File | Meaning | Effect on verdict |
|---|---|---|
| `strong.txt` | Confirmed PolinRider artifacts | `INFECTED` |
| `bad-packages.txt` | Malicious package names from the campaign | `INFECTED` |
| `filenames.txt` | Paths that are indicators on their own (extended regex) | `INFECTED` |
| `weak.txt` | Strings legitimate projects also use | `review` only |
| `network.txt` | Campaign hosts and IP addresses | `INFECTED` if a live connection matches |
| `implant-paths.txt` | Second-stage install, persistence and working paths | `INFECTED` if the path exists |
| `implant-names.txt` | Implant binary and process names | `INFECTED` on an exact process-name match |
| `hashes.txt` | SHA-256 of known implant binaries | `INFECTED` on a hash match, whatever the file is named |

Format: one entry per line. Lines starting with `#` and blank lines are ignored.
`strong.txt`, `bad-packages.txt` and `weak.txt` are matched as fixed strings
(`grep -F`), so no regex escaping is needed. `filenames.txt` is extended regex
matched against the full path.

## The classification rule

Put an indicator in `strong.txt` **only if a match means infection with no
plausible alternative.** If a legitimate project could contain the same string,
it belongs in `weak.txt`.

A false positive in `strong.txt` costs every user of this repository a panic. A
miss in `weak.txt` costs one line of review output. When the two are close,
choose `weak.txt`.

## Keeping it current

The campaign rotates its signatures, so an indicator set that matched in
September will not necessarily match in December. The set is reviewed weekly
against the published sources, and **every review is recorded, including the
weeks where nothing changed**, in
[`docs/indicator-reviews/`](https://github.com/meSingh/polinrider-cleaner/tree/main/docs/indicator-reviews).

A scanner that has quietly stopped matching is worse than no scanner, because it
returns a clean result somebody believes. The review log is how you check that
has not happened.

## Self-matching

Any file that *detects* PolinRider contains PolinRider strings by definition.
Your own scanners, this repository, and CI workflows built from it will be
flagged by a grep-based scan. That is expected, and the triage filter separates
those matches by path.
