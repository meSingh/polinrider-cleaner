#!/usr/bin/env python3
"""Keep a history of release downloads, with our own downloads kept apart.

GitHub keeps one running total per release file and no history, so on its own
it cannot say when anything was downloaded or by whom. Run once a day by
.github/workflows/traffic.yml, this records every file's total, and from the
day-to-day change works out downloads per week. Downloads we made ourselves
(checking a release, a test machine, a package manager's validation) are
recorded in ours.csv and taken off, so what is left is people.

Usage:
    downloads.py RELEASES_JSON DATA_DIR OUT_DIR --today YYYY-MM-DD
                 [--start-ours CSV] [--record TAG FILES COUNT DATE NOTE]

RELEASES_JSON is the releases API's answer (every page, merged into one
array). DATA_DIR holds counts.csv and ours.csv as they are on the traffic
branch now, either missing on the first run. OUT_DIR receives counts.csv,
ours.csv, weekly.csv, weekly-by-platform.csv, summary.md and README.md.

counts.csv gets a row for a file only when its total has moved since the last
row (or the file is new), so it grows with downloads, not with days.

--start-ours is the ledger to begin from when DATA_DIR has no ours.csv: the
downloads of ours made before the log existed.

--record adds rows to ours.csv. FILES is "all" or a comma-separated list of
file names, each of which may use * as a wildcard. Every file named must exist
in that release, the date must fall between the release's publication and
today, and the same rows must not be recorded already, or nothing is written.

Standard library only: the runner's Python, nothing installed.
"""

import argparse
import csv
import datetime as dt
import fnmatch
import json
import os
import sys
from collections import defaultdict

COUNTS_HEADER = ["date", "tag", "file", "total"]
OURS_HEADER = ["date", "tag", "file", "count", "note"]

README = """\
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
"""


def fail(message):
    print(f"downloads.py: {message}", file=sys.stderr)
    sys.exit(1)


def read_csv(path, header):
    if not os.path.exists(path) or os.path.getsize(path) == 0:
        return []
    with open(path, newline="") as f:
        reader = csv.reader(f)
        found = next(reader)
        if found != header:
            fail(f"{path} has columns {found}, expected {header}")
        rows = []
        for line, values in enumerate(reader, start=2):
            if len(values) != len(header):
                fail(f"{path} line {line} has {len(values)} fields, expected {len(header)}")
            rows.append(dict(zip(header, values)))
    return rows


def write_csv(path, header, rows):
    with open(path, "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(header)
        for r in rows:
            w.writerow([r[h] for h in header] if isinstance(r, dict) else r)


def parse_day(text, what):
    try:
        return dt.date.fromisoformat(text)
    except (TypeError, ValueError):
        fail(f"{what} {text!r} is not a date (YYYY-MM-DD)")


def parse_count(text, what):
    try:
        n = int(text)
    except (TypeError, ValueError):
        fail(f"{what} {text!r} is not a whole number")
    if n < 0:
        fail(f"{what} {text!r} is below zero")
    return n


def week_of(day):
    """The Monday that starts the ISO week holding this day."""
    return day - dt.timedelta(days=day.weekday())


def is_verification(name):
    """Checksums and attestations: fetched to check a download, not to run it."""
    return name == "SHA256SUMS" or name.endswith(".intoto.jsonl")


def platform_of(tag, name):
    if is_verification(name):
        return "checksums and attestations"
    if tag.startswith("v1."):
        return "1.x shell"
    # polinrider-v2.3.0-macos-arm64.tar.gz -> macos-arm64.tar.gz
    rest = name.split(tag, 1)[1] if tag in name else name
    return rest.lstrip("-.") or name


def load_releases(path):
    try:
        with open(path) as f:
            releases = json.load(f)
    except (OSError, ValueError) as e:
        fail(f"cannot read the releases from {path}: {e}")
    ok = isinstance(releases, list) and all(
        isinstance(r, dict) and isinstance(r.get("tag_name"), str)
        and isinstance(r.get("assets"), list)
        and all(isinstance(a, dict) and isinstance(a.get("name"), str)
                and isinstance(a.get("download_count"), int) for a in r["assets"])
        for r in releases)
    if not ok:
        fail(f"{path} is not a list of releases with assets")
    return [r for r in releases if not r.get("draft")]


def record_ours(releases, ours, today, tag, files, count, day, note):
    release = next((r for r in releases if r["tag_name"] == tag), None)
    if release is None:
        fail(f"no release is tagged {tag!r}")
    names = [a["name"] for a in release["assets"]]
    if files.strip() == "all":
        chosen = names
    else:
        chosen = []
        for pattern in (p.strip() for p in files.split(",")):
            hits = [n for n in names if pattern and fnmatch.fnmatchcase(n, pattern)]
            if not hits:
                fail(f"{pattern!r} matches no file in {tag}; its files are: {', '.join(names)}")
            chosen += [n for n in hits if n not in chosen]
    n = parse_count(count, "count")
    if n < 1:
        fail("count must be at least 1")
    note = " ".join(note.split())
    if not note:
        fail("a note is required: who downloaded and why")
    when = parse_day(day, "date")
    published = release.get("published_at") or ""
    if published and when < parse_day(published[:10], "publication date"):
        fail(f"{tag} was published on {published[:10]}, after {day}")
    if when > today:
        fail(f"{day} is after today ({today})")
    new = [{"date": day, "tag": tag, "file": f, "count": str(n), "note": note} for f in chosen]
    seen = {(r["date"], r["tag"], r["file"], r["note"]) for r in ours}
    again = [r["file"] for r in new if (r["date"], r["tag"], r["file"], r["note"]) in seen]
    if again:
        fail(f"already recorded on {day} with this note: {', '.join(again)}. "
             "For more downloads of the same files, record them with a different note.")
    ours += new
    print(f"Recorded {n} of our own download(s) of {len(chosen)} file(s) from {tag} on {day}.")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("releases")
    ap.add_argument("data_dir")
    ap.add_argument("out_dir")
    ap.add_argument("--today", required=True)
    ap.add_argument("--start-ours")
    ap.add_argument("--record", nargs=5, metavar=("TAG", "FILES", "COUNT", "DATE", "NOTE"))
    args = ap.parse_args()

    today = parse_day(args.today, "--today")
    releases = load_releases(args.releases)

    counts = read_csv(os.path.join(args.data_dir, "counts.csv"), COUNTS_HEADER)
    ours_path = os.path.join(args.data_dir, "ours.csv")
    if not os.path.exists(ours_path) and args.start_ours:
        if not os.path.exists(args.start_ours):
            fail(f"--start-ours {args.start_ours} does not exist")
        ours_path = args.start_ours
    ours = read_csv(ours_path, OURS_HEADER)
    for r in counts:
        parse_day(r["date"], f"counts.csv date for {r['tag']} {r['file']}")
        parse_count(r["total"], f"counts.csv total for {r['tag']} {r['file']}")
    for r in ours:
        parse_day(r["date"], f"ours.csv date for {r['tag']} {r['file']}")
        parse_count(r["count"], f"ours.csv count for {r['tag']} {r['file']}")
    if counts and max(r["date"] for r in counts) > today.isoformat():
        fail(f"counts.csv already has totals after {today}; is --today right?")

    if args.record:
        record_ours(releases, ours, today, *args.record)

    # Today's totals replace any taken earlier today, so a second run in a day
    # (a manual one, say) updates rather than duplicates. A row is added only
    # for a new file or a total that moved.
    counts = [r for r in counts if r["date"] != today.isoformat()]
    counts.sort(key=lambda r: (r["date"], r["tag"], r["file"]))
    latest = {(r["tag"], r["file"]): int(r["total"]) for r in counts}
    for r in releases:
        for a in r["assets"]:
            key = (r["tag_name"], a["name"])
            if latest.get(key) != a["download_count"]:
                counts.append({"date": today.isoformat(), "tag": key[0],
                               "file": key[1], "total": str(a["download_count"])})
    counts.sort(key=lambda r: (r["date"], r["tag"], r["file"]))
    ours.sort(key=lambda r: (r["date"], r["tag"], r["file"]))

    # Downloads per file per week. A total taken on day D counts downloads
    # since the run before, mostly made on D-1, so the change is put on D-1.
    # Everything in the first totals has no date: it goes in "before".
    first_day = parse_day(counts[0]["date"], "date") if counts else today
    before = f"before {first_day.isoformat()}"
    last = {}
    rows = []  # (bucket, tag, file, downloads)
    for r in counts:
        key = (r["tag"], r["file"])
        day = parse_day(r["date"], "date")
        total = int(r["total"])
        if day == first_day:
            change, bucket = total, before
        elif key in last:
            # A total below the last one means the file was replaced, and
            # the count started again from nothing.
            change = total - last[key] if total >= last[key] else total
            bucket = week_of(day - dt.timedelta(days=1)).isoformat()
        else:
            # A file first seen after the log began was published since the
            # run before, so all of its downloads are new.
            change, bucket = total, week_of(day - dt.timedelta(days=1)).isoformat()
        last[key] = total
        if change:
            rows.append((bucket, r["tag"], r["file"], change))

    # One of our downloads on day X is in the first totals when X is before
    # the first run's day, and otherwise in the week holding X.
    def bucket_of_ours(day):
        d = parse_day(day, "date")
        return before if d < first_day else week_of(d).isoformat()

    weekly = defaultdict(lambda: [0, 0, 0, 0])  # downloads, ours, verification, verification ours
    by_platform = defaultdict(lambda: [0, 0])
    for bucket, tag, name, n in rows:
        weekly[bucket][2 if is_verification(name) else 0] += n
        by_platform[(bucket, platform_of(tag, name))][0] += n
    for r in ours:
        bucket, n = bucket_of_ours(r["date"]), int(r["count"])
        weekly[bucket][3 if is_verification(r["file"]) else 1] += n
        by_platform[(bucket, platform_of(r["tag"], r["file"]))][1] += n

    def order(bucket):
        return "" if bucket == before else bucket

    out = args.out_dir
    os.makedirs(out, exist_ok=True)
    write_csv(os.path.join(out, "counts.csv"), COUNTS_HEADER, counts)
    write_csv(os.path.join(out, "ours.csv"), OURS_HEADER, ours)
    write_csv(os.path.join(out, "weekly.csv"),
              ["week", "downloads", "ours", "people", "checksums_and_attestations"],
              [[b, d, o, d - o, v - vo] for b, (d, o, v, vo) in
               sorted(weekly.items(), key=lambda x: order(x[0]))])
    write_csv(os.path.join(out, "weekly-by-platform.csv"),
              ["week", "platform", "downloads", "ours", "people"],
              [[b, p, d, o, d - o] for (b, p), (d, o) in
               sorted(by_platform.items(), key=lambda x: (order(x[0][0]), x[0][1]))])
    with open(os.path.join(out, "README.md"), "w") as f:
        f.write(README)

    # A short table for the run's summary page: the last eight weeks, then
    # everything before the log began, then all of it.
    weeks = sorted((b for b in weekly if b != before), reverse=True)[:8]
    lines = ["| Week from | Downloads | Ours | People |", "|---|---:|---:|---:|"]
    for b in weeks + ([before] if before in weekly else []):
        d, o, _, _ = weekly[b]
        lines.append(f"| {b} | {d} | {o} | {d - o} |")
    d, o = sum(w[0] for w in weekly.values()), sum(w[1] for w in weekly.values())
    lines.append(f"| **All** | **{d}** | **{o}** | **{d - o}** |")
    note = ("Binaries and packages only; checksums and attestations are in weekly.csv. "
            "People can read below zero for a week when one of our downloads is "
            "recorded in a different week from the one GitHub counted it in.")
    with open(os.path.join(out, "summary.md"), "w") as f:
        f.write("### Release downloads\n\n" + "\n".join(lines) + "\n\n" + note + "\n")


if __name__ == "__main__":
    main()
