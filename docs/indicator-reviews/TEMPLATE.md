# Indicator review — YYYY-MM-DD

| | |
|---|---|
| **Outcome** | one line: `no change`, or `N indicators added` |
| **Pull request** | #NNN, or `none` |
| **Indicator set** | unchanged, or the files touched |

## Sources

| Source | Reached | What it said |
|---|---|---|
| OpenSourceMalware dossier | | |
| opensourcemalware.com blog | | |
| Web search, last 7 days | | |
| socket.dev tracker | | |

Say plainly which sources failed and why. A review that could not reach half its
sources is a weaker claim than one that reached all of them, and the reader
cannot tell the difference unless the entry says so.

## Upstream state

What the dossier and the published coverage look like as of this date. Revision
dates, latest commit, whether the signatures have rotated again.

## Added

One row per indicator, with the source link for each. Delete the section if
nothing was added — do not leave it empty with a "none" in it.

| Indicator | File | Why there | Source |
|---|---|---|---|
| | | | |

## Considered and not added

Everything the review found and rejected, with the reason. Keep this even when
the review added nothing: a rejection recorded once saves the next reviewer from
re-deriving it, and a rejection that turns out to be wrong is only findable if
it was written down.

## Open gaps

Anything known to be missing from `ioc/` that this review did not close, and what
closing it would take. Carry a gap forward from the previous entry until it is
closed, so it does not quietly disappear.

## Notes

Anything else the next reviewer needs: environment problems, a source that moved,
a technique that needs a scanner change rather than an indicator, a page that
tried to give the reviewer instructions.
