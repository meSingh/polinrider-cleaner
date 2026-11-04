#!/usr/bin/env python3
"""Keep a history of release downloads, with our own downloads kept apart.

GitHub keeps one running total per release file and no history, so on its own
it cannot say when anything was downloaded or by whom. Run once a day by
.github/workflows/traffic.yml, this records every file's total, and from the
day-to-day change works out downloads per week. Downloads we made ourselves
(checking a release, a test machine, a package manager's validation) are
recorded by hand in ours.csv and taken off, so what is left is people.

Usage:
    downloads.py RELEASES_JSON DATA_DIR OUT_DIR --today YYYY-MM-DD
                 [--record TAG FILES COUNT DATE NOTE]

RELEASES_JSON is the releases API's answer (every page, merged into one
array). DATA_DIR holds the files as they are on the traffic branch now, any of
them missing on the first run. OUT_DIR receives counts.csv, ours.csv,
weekly.csv, weekly-by-platform.csv and summary.md.

--record adds a row to ours.csv. FILES is "all" or a comma-separated list of
file names, each of which may use * as a wildcard. Every file named must exist
in that release, or nothing is written.

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


def fail(message):
    print(f"downloads.py: {message}", file=sys.stderr)
    sys.exit(1)


def read_csv(path, header):
    if not os.path.exists(path) or os.path.getsize(path) == 0:
        return []
    with open(path, newline="") as f:
        rows = list(csv.DictReader(f))
    if rows and list(rows[0].keys()) != header:
        fail(f"{path} has columns {list(rows[0].keys())}, expected {header}")
    return rows


def write_csv(path, header, rows):
    with open(path, "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(header)
        for r in rows:
            w.writerow([r[h] if isinstance(r, dict) else r[i] for i, h in enumerate(header)])


def parse_day(text, what):
    try:
        return dt.date.fromisoformat(text)
    except ValueError:
        fail(f"{what} {text!r} is not a date (YYYY-MM-DD)")


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


def record_ours(releases, ours, tag, files, count, day, note):
    release = next((r for r in releases if r["tag_name"] == tag), None)
    if release is None:
        fail(f"no release is tagged {tag}")
    names = [a["name"] for a in release["assets"]]
    if files.strip() == "all":
        chosen = names
    else:
        chosen = []
        for pattern in (p.strip() for p in files.split(",")):
            hits = [n for n in names if fnmatch.fnmatchcase(n, pattern)]
            if not hits:
                fail(f"{pattern!r} matches no file in {tag}; its files are: {', '.join(names)}")
            chosen += [n for n in hits if n not in chosen]
    try:
        n = int(count)
    except ValueError:
        fail(f"count {count!r} is not a whole number")
    if n < 1:
        fail("count must be at least 1")
    if not note.strip():
        fail("a note is required: who downloaded and why")
    parse_day(day, "date")
    for name in chosen:
        ours.append({"date": day, "tag": tag, "file": name, "count": str(n), "note": note.strip()})
    print(f"Recorded {n} of our own download(s) of {len(chosen)} file(s) from {tag} on {day}.")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("releases")
    ap.add_argument("data_dir")
    ap.add_argument("out_dir")
    ap.add_argument("--today", required=True)
    ap.add_argument("--record", nargs=5, metavar=("TAG", "FILES", "COUNT", "DATE", "NOTE"))
    args = ap.parse_args()

    today = parse_day(args.today, "--today")
    with open(args.releases) as f:
        releases = json.load(f)
    releases = [r for r in releases if not r.get("draft")]

    counts = read_csv(os.path.join(args.data_dir, "counts.csv"), COUNTS_HEADER)
    ours = read_csv(os.path.join(args.data_dir, "ours.csv"), OURS_HEADER)

    if args.record:
        record_ours(releases, ours, *args.record)

    # Today's totals replace any taken earlier today, so a second run in a day
    # (a manual one, say) updates rather than duplicates.
    counts = [r for r in counts if r["date"] != today.isoformat()]
    for r in releases:
        for a in r["assets"]:
            counts.append({"date": today.isoformat(), "tag": r["tag_name"],
                           "file": a["name"], "total": str(a["download_count"])})
    counts.sort(key=lambda r: (r["date"], r["tag"], r["file"]))
    ours.sort(key=lambda r: (r["date"], r["tag"], r["file"]))

    # Downloads per file per day. A total taken in the early hours of day D
    # mostly counts downloads made on D-1, so the change is put on D-1.
    # Everything before the first total has no date: it goes in "before".
    first_day = parse_day(counts[0]["date"], "date") if counts else today
    before = f"before {first_day.isoformat()}"
    last = {}
    rows = []  # (bucket, tag, file, downloads)
    for r in counts:
        key = (r["tag"], r["file"])
        day = parse_day(r["date"], "date")
        total = int(r["total"])
        if key in last:
            change, bucket = total - last[key], week_of(day - dt.timedelta(days=1)).isoformat()
        elif day == first_day:
            change, bucket = total, before
        else:
            # A file first seen after the log began was published since the
            # previous total, so all of its downloads are new.
            change, bucket = total, week_of(day - dt.timedelta(days=1)).isoformat()
        last[key] = total
        if change:
            rows.append((bucket, r["tag"], r["file"], change))

    def bucket_of_ours(day):
        d = parse_day(day, "date")
        return before if d < first_day - dt.timedelta(days=1) else week_of(d).isoformat()

    weekly = defaultdict(lambda: [0, 0, 0, 0])  # downloads, ours, verification, verification ours
    by_platform = defaultdict(lambda: [0, 0])
    for bucket, tag, name, n in rows:
        slot = 2 if is_verification(name) else 0
        weekly[bucket][slot] += n
        by_platform[(bucket, platform_of(tag, name))][0] += n
    for r in ours:
        bucket, n = bucket_of_ours(r["date"]), int(r["count"])
        slot = 3 if is_verification(r["file"]) else 1
        weekly[bucket][slot] += n
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

    # A short table for the run's summary page: the last eight weeks.
    weeks = sorted((b for b in weekly if b != before), reverse=True)[:8]
    lines = ["| Week from | Downloads | Ours | People |", "|---|---:|---:|---:|"]
    for b in weeks:
        d, o, _, _ = weekly[b]
        lines.append(f"| {b} | {d} | {o} | {d - o} |")
    if before in weekly:
        d, o, _, _ = weekly[before]
        lines.append(f"| {before} | {d} | {o} | {d - o} |")
    note = ("Binaries and packages only. Checksums and attestations are in weekly.csv. "
            "People can read below zero for a week when one of our downloads is "
            "recorded in a different week from the one GitHub counted it in.")
    with open(os.path.join(out, "summary.md"), "w") as f:
        f.write("### Release downloads\n\n" + "\n".join(lines) + "\n\n" + note + "\n")


if __name__ == "__main__":
    main()
