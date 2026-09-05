# Indicator set

One source of truth. Every script in this repository reads these files at runtime,
so adding an indicator here updates the org scanner, the personal scanner and all
three local checks at once.

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

`network.txt` is used only by the local checks, to compare established
connections from `node` and Electron processes against known campaign
infrastructure. It is also the list to feed an egress blocklist. The IP addresses
rotate, so block them but do not rely on them.

Format: one entry per line. Lines starting with `#` and blank lines are ignored.
`strong.txt`, `bad-packages.txt` and `weak.txt` are matched as fixed strings
(`grep -F`), so no regex escaping is needed. `filenames.txt` is extended regex
matched against the full path.

`implant-names.txt` is matched against the **process name only**, never the full
command line. Matching a command line would report this scanner, and anyone
grepping for the implant, as the implant itself.

## Adding an indicator

Put it in `strong.txt` only if a match means infection with no plausible
alternative. If a legitimate project could contain the same string, it belongs in
`weak.txt`. A false positive in `strong.txt` costs every user of this repository a
panic; a miss in `weak.txt` costs one line of review output.

Current published tracking for this campaign:
<https://socket.dev/supply-chain-attacks/polinrider>

## Self-matching

Any file that *detects* PolinRider contains PolinRider strings by definition. Your
own scanners, this repository, and CI workflows built from it will be flagged by a
grep-based scan. That is expected. `triage-filter.sh` separates those matches by
path; see `org-cleanup/README.md`, step 3.
