# Indicator reviews

<sub>[← back to the docs index](../README.md) · the indicator set itself is
[`../../ioc/`](../../ioc/)</sub>

---

This campaign rotates its signatures. An indicator set that matched in September
will not necessarily match in December, and a scanner that has quietly stopped
matching is worse than no scanner at all, because it returns a clean result that
somebody believes.

So the set is reviewed every week against the published sources, and each review
gets its own dated file in this directory. **Entries are never edited or
deleted.** A review records what was known on one day; correcting it later
destroys the only thing it is good for. If a later review contradicts an earlier
one, the later entry says so and both stay.

The weeks that find nothing are the point. A run of *no change* entries is a
claim about the campaign. A gap in the dates is a claim about nobody checking.

## Reviews

Newest first.

| Date | Outcome | What it found |
|---|---|---|
| [2026-10-05](2026-10-05.md) | 83 indicators added | A second analyst team decoded the Ethereum dead drop end to end: two operator wallets, eleven C2 servers, 35 live repositories. It also showed that the `0xa322` wallet address in `strong.txt` has been in the wrong case since it was added, and settled the XOR-key boundary the 2026-09-28 entry could not. 55 package names lifted from the OpenSSF malicious-packages database, which turns out to be readable from here, five rejected as substrings of live packages. First current-variant font indicator. Two self-tests that failed for four weeks now pass |
| [2026-09-28](2026-09-28.md) | 11 indicators added | The campaign is A/B testing three fake-font variants at once. Added the single-quoted `A8` marker, two XOR keys, the `.gitignore` machine fingerprint and `allowAutomaticTasks`. Reversed last week's rejection of the C2 paths: this build leaves them in plain text. The font indicators are a variant behind and cannot catch up as fixed strings. |
| [2026-09-21](2026-09-21.md) | 8 indicators added | The `A8-` campaign marker, five Ethereum RPC resolvers and two NullReceiver C2 addresses read off the chain. CloudSEK's GHAPPIER report rejected as a separate loader family sharing victims, not infrastructure. Found one PolinRider post the previous week's sweep missed. |
| [2026-09-14](2026-09-14.md) | 19 indicators added | The egress block on the analysis hosts lifted. Drained a backlog: the ChainVeil and ViteVenom npm clusters, the NullReceiver marker, wallet and C2. Three known-malicious names rejected as substrings of real packages. npm-CLI persistence found, not implemented. |
| [2026-09-07](2026-09-07.md) | 3 indicators added | Two npm package names and one BSC RPC node, all gaps against the April 2026 dossier. Nothing new published upstream. |

## Sources checked every week

| Source | What it is | Reachable from the review environment |
|---|---|---|
| [OpenSourceMalware/PolinRider](https://github.com/OpenSourceMalware/PolinRider) | the primary dossier: README, YARA rules, CSV data drops | yes, by git clone |
| [opensourcemalware.com blog](https://opensourcemalware.com/blog) | the team's running coverage | yes, since 2026-09-14. Fetch a **post path** — the bare domain and `/blog/rss.xml` return an empty single-page-app shell, though `/blog` itself now server-renders the full post index. Enumerate every slug containing `polinrider` and check it off; following the posts a previous entry named misses the ones it missed |
| Web search | anything published in the last seven days, searched under the campaign name and under `MicrosoftSystem64` and `ForceMemo`, where new analysis often lands first | yes |
| [socket.dev tracker](https://socket.dev/supply-chain-attacks/polinrider) | current package counts | no, and it refuses automated fetches regardless |
| [safedep.io](https://safedep.io/) | the second team publishing primary teardowns of this campaign. Its article pages embed the indicator table verbatim in a `data-csv` attribute, which is a better source than the rendered HTML | yes, since first tested 2026-10-05. Needs `curl -L`: the un-slashed URL returns 308 |
| [OpenSSF `malicious-packages`](https://github.com/ossf/malicious-packages) | the machine-readable malicious-package database. Searching it for indicators already in `ioc/` finds the registry side of the campaign | yes, by git clone. Expect ~2 GB checked out, and do **not** use `--filter=blob:none` |
| [registry.npmjs.org](https://registry.npmjs.org/) and [pypi.org](https://pypi.org/) | not sources of indicators — the way to check one. A name going into `bad-packages.txt` must not be a substring of a live package, and these answer that mechanically | yes, since first tested 2026-10-05, including the full `pypi.org/simple/` index |

The blocked hosts are a property of the sandbox the weekly job runs in, not of
the sources. Their content still reaches a review through search summaries,
which is enough to establish that something was published and roughly what it
said. It is **not** enough to lift an indicator from, and no entry here does.

As of 2026-09-14 the sandbox's egress allowlist has widened, and most of those
hosts — `opensourcemalware.com`, `checkmarx.com`, `stepsecurity.io`,
`securityonline.info`, `thehackernews.com` — now serve full content. `socket.dev`
still does not, and that one is a policy of the site rather than of the sandbox.
`medium.com` does not either, which cost the 2026-10-05 review a SlowMist report.
Re-check reachability every week regardless of what this table says; it has
changed once and can change back — and test hosts the table does not list, which
is how three of the rows above were found.

## What a review looks for

Two different things, with two different destinations.

| Found | Goes to | Decided by |
|---|---|---|
| **An indicator** — a string, package name, host, address, hash, implant path, process name | a line in [`ioc/`](../../ioc/) | the reviewer, following the classification rule below |
| **A technique** — a change in how the campaign spreads, persists or hides | a new check in the scanner | the maintainer. A review describes it and stops |

The classification rule, from [`ioc/README.md`](../../ioc/README.md) and not
negotiable: `strong.txt` only if a match means infection with no plausible
alternative; `weak.txt` if a legitimate project could contain the same string.
A false positive in `strong.txt` costs every user of this repository a panic. A
miss in `weak.txt` costs one line of review output. When the two are close,
choose `weak.txt`.

Nothing is ever invented. Every indicator traces to a named public source that
the reviewer actually read.

## Adding an entry

1. Copy [`TEMPLATE.md`](TEMPLATE.md) to `YYYY-MM-DD.md`, dated in Asia/Kolkata.
2. Fill every section. The **considered and not added** section is not optional
   and is usually the most useful part: it is what stops the next reviewer
   re-deriving the same rejection.
3. Add a row to the table above, newest first.
4. Update the **Indicator review** section at the end of the
   [main README](../../README.md) with the new date and a two-line summary.
