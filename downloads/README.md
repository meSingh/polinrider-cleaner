# Release downloads

Written daily by .github/workflows/traffic.yml on main, by
.github/scripts/downloads.py. Data only.

- `counts.csv`: each release file's running total as GitHub reports it. A row
  is written only when a total moves, so a file's last row is its total now.
- `ours.csv`: downloads we made ourselves (release checks, test machines,
  package-manager validation), one row per file. Recorded with the workflow's
  own_* inputs; see "Traffic log" in AGENTS.md.
- `weekly.csv`: per week (Monday to Sunday), binaries and packages:
  `downloads` as GitHub counted them, `ours`, and `people`, the difference.
  `checksums_and_attestations` is people's downloads of SHA256SUMS and the
  attestation bundle, kept out of the other columns because they are fetched
  to check a download, not to run anything.
- `weekly-by-platform.csv`: the same split by file, all files included.
- `summary.md`: the last eight weeks, as shown on the workflow run.

The first row of each table is everything before the log began, which has no
dates. A total taken on a day is put on the day before, when most of what it
counts happened. "People" can read below zero for a week when one of our
downloads is recorded in a different week from the one GitHub counted it in.
