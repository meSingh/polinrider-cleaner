# 0053. Count release downloads daily, with our own kept apart

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-11-04 |

## Context

GitHub keeps one running total per release file and no history. It cannot say
when a file was downloaded or by whom, so a week after a post or a release
there is no way to tell whether anyone came. And some of the totals could be
us: every release is checked by downloading all eleven files, the winget
manifest check fetches the Windows zip and `SHA256SUMS`, test machines fetch
binaries. The release and winget checks went through `gh release download`,
which GitHub does not count (below), so none of the 126 downloads made by
2026-11-04, checksums and attestations aside, is ours by any record kept.
v2.3.0's eleven files each read 1 the morning after its release, after its
release check; those were someone else's.

The Traffic log workflow (`traffic.yml`) already ran daily and kept what GitHub
forgets on the `traffic` branch, so the totals belong there.

Which of our downloads GitHub counts was tested, not assumed. v2.3.0's
`SHA256SUMS` read 1 on 2026-11-04 after two downloads: the release workflow's
`packages` job (`gh release download`, 19:14 UTC on 3 November) and the
release check. On 4 November, one download of it through `gh release
download` at 04:43 UTC, then two from its `releases/download/` link with
`curl` at 04:51 and 05:28, moved the count from 1 to 2 between 04:57 and
05:02 and to 3 between 05:37 and 05:42, and it was still 3 at 05:43. The two link
downloads counted, each about ten minutes later; the API download never did.
A second test confirmed it on another file: a `gh release download` of
v2.2.0's `polinrider-v2.2.0.intoto.jsonl` at 05:45 UTC left its count at 2 for
an hour, and one `curl` download from its link at 06:46 moved it to 3 between
06:56 and 07:01. So a download through the asset's API URL, which is what
`gh release download` fetches, is not counted, and the count lags by minutes,
not days. The `packages` job's fetch, our earlier release checks and the
winget check all went through the API, so none of them is recorded as ours,
and neither are the two test downloads made with `gh`; the three `curl`
downloads are.

## Decision

`.github/scripts/downloads.py`, run daily by `traffic.yml`, records every
release file's total under `downloads/` on the `traffic` branch and works out
downloads per week from the day-to-day change. A row is written to
`counts.csv` only when a total moves, so the file grows with downloads, not
with days. The change found on day D is put on D-1, when most of what it
counts happened. Everything in the first totals goes in one "before" row,
because it has no dates.

Our own downloads are a ledger, `ours.csv`, written by running the workflow
by hand with the `own_*` inputs, and taken off each week to leave "people".
The ledger starts from `.github/scripts/downloads-ours-start.csv`, the
downloads of ours that GitHub counted before the log existed: the three `curl`
test downloads above. Checksums and attestations
are kept out of the main columns: they are fetched to check a download, not
to run anything.

It is Python, standard library only, because the runner has it and the
arithmetic over CSV files is unpleasant in shell. It is not part of the tool
and is never shipped. The download part reads releases with the workflow
token, so it runs whether or not `TRAFFIC_TOKEN` is set, and writes all its
files in one commit through the git data API, signed by GitHub like the rest
of the branch.

## Consequences

The weekly number of people is visible, release checks no longer read as
interest, and a launch or a post can be measured against the weeks before it.

What it makes worse, and where it is wrong:

- **The ledger is only as good as our memory.** A download of ours that is
  not recorded reads as a person. Nothing can detect it.
- **The counting rule is GitHub's, and observed, not documented.** If GitHub
  starts counting API downloads, the `packages` job's fetch will read as a
  person once per release, and so would every release check made with
  `gh release download`, none of which is recorded. Nothing here would notice. The test above is cheap to repeat.
- **Weeks are approximate at the edges.** The scheduled run is meant for
  03:17 UTC but GitHub starts it hours late (between 07:45 and 11:05 UTC
  from 15 October to 3 November 2026), and GitHub's count itself lags. A download near midnight
  on a Sunday can land in either week, and one of ours can be recorded in a
  different week from the one GitHub counted it in, so "people" can read
  below zero for a week. Totals over several weeks are right.
- **The "before" row is one lump.** The 126 downloads from September to
  November 2026 have no dates and never will.
- **Bots and mirrors count as people.** GitHub does not say who downloaded,
  and neither can this.
- A run that cannot read the branch stops rather than starting the history
  again, so a GitHub outage loses that day's total, not the history. The
  next day's change then covers two days.

## Related

- ADR-0043, packaging: Homebrew, Scoop and winget fetch the release files
  this counts.
