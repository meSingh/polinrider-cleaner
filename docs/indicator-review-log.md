# Indicator review log

<sub>[← back to the docs index](README.md) · the indicator set itself is
[`../ioc/`](../ioc/)</sub>

---

This campaign rotates its signatures. An indicator set that matched in September
will not necessarily match in December, and a scanner that has quietly stopped
matching is worse than no scanner at all, because it returns a clean result that
somebody believes.

So the set is reviewed every week against the published sources, and every review
is recorded here — including the weeks that found nothing. A week with no change
is the useful record: it says the set was looked at on that date, not that it was
forgotten. Read a long run of "no change" rows as a claim about the campaign, and
a gap in the dates as a claim about nobody checking.

## Sources checked each week

| Source | What it is |
|---|---|
| [OpenSourceMalware/PolinRider](https://github.com/OpenSourceMalware/PolinRider) | the primary dossier: README, YARA rules, and the CSV data drops |
| [opensourcemalware.com blog](https://opensourcemalware.com/blog/rss.xml) | the team's running coverage, checked back eight days |
| Web search | coverage from the last seven days, searched under the campaign name and under the second-stage names `MicrosoftSystem64` and `ForceMemo`, which is where new analysis often lands |
| [socket.dev tracker](https://socket.dev/supply-chain-attacks/polinrider) | read through coverage that quotes it; the page itself refuses automated fetches |

A review looks for two different things. New **indicators** — strings, package
names, hosts, addresses, hashes, implant paths, process names — become lines in
`ioc/`. New **techniques** — a change in how the campaign spreads, persists or
hides — usually need a new check in the scanner instead, and that is a decision
for the maintainer, not something a review should implement on its own.

## Log

| Week of | Upstream change | Change to `ioc/` |
|---|---|---|
| 2026-09-07 | none published in the window; latest coverage is July 2026 | 3 indicators added, from gaps against the April 2026 dossier |

### 2026-09-07

**Sources reached:** the dossier repository (cloned and read in full).
**Sources not reached:** `opensourcemalware.com` and every third-party analysis
host were unreachable from the review environment, which permits outbound HTTPS
only to an allowlist. Their content was read through search summaries instead,
which is enough to establish that something was published and roughly what it
said, and is not enough to copy an indicator out of. Nothing in this week's
change is sourced that way.

**Nothing new was published this week.** The dossier README still carries its
2026-04-11 revision date. Its most recent commit is 2026-07-09, adding
`polinrider-compromised-owners-july10.csv` and
`polinrider-compromised-repos-july10.csv`: 2,417 victim repositories and 1,219
owners. Those are victim inventories, not indicators — the injection vectors and
hit paths in them are the four already documented (config-file injection, fake
`.woff2` font, `.vscode/tasks.json`, malicious npm dependency), and no new marker,
host or path appears in either file.

**Three gaps were closed against the April 2026 dossier**, all of them things
this repository should have carried from the start rather than anything the
campaign did this week:

- `tailwind-animationbased` and `tailwindcss-animate-style` → `ioc/bad-packages.txt`.
  Both appear in the dossier's npm package table from the same deleted publisher
  accounts as the five names already listed. Both had zero observed victim
  repositories at the time of the hunt, which is presumably why they were left
  out; a ghost dependency reference in a `package.json` outlives the registry
  entry, so the names are still worth matching.
- `bsc-rpc.publicnode.com` → `ioc/weak.txt`. The loader's second BNB Smart Chain
  RPC node, listed beside `bsc-dataseed` in the dossier. It is a public endpoint
  that legitimate projects also use, so it is a review signal and not a verdict.

**Found and deliberately not added:**

- The obfuscator shuffle seeds `2857687`, `2667686` (variant 1) and `1111436`,
  `3896884` (variant 2). The dossier's YARA rule uses them, but only in
  conjunction with a marker or a decoder name. On their own they are seven-digit
  numbers, and `ioc/` is matched as fixed strings with no conjunction, so each
  one would flag any file that happens to contain that number.
- `global['r'] = require` and `global['m'] = module`, the two strings the dossier
  YARA rule treats as common across both variants. They would be the way to catch
  a third variant that rotates its marker, which makes them tempting, but the
  exact spacing is an artefact of one generator's output and minification breaks
  the match. A regex check in the scanner would be the right shape for this, not
  a fixed string in `ioc/`.

**Open gap, needing a maintainer decision.** In early July 2026 the campaign was
reported to have expanded well beyond the GitHub-injection and npm activity this
repository covers: Socket counted 162 malicious release artifacts across 108
unique packages and extensions, spanning npm, Packagist, Go modules and one
Chrome extension, and Checkmarx documented two npm clusters — ChainVeil and
ViteVenom, the latter scoped names imitating `@vitejs/*` — which the
OpenSourceMalware team then attributed to this same actor. The main README's
scale table already records those counts; what is missing is the names, none of
which are in `ioc/`, so the scanner cannot match a single one of them.

The package lists were not retrievable from the review environment, and
`ioc/` holds only indicators traceable to a source that was actually read, so
nothing from it has been added here. Retrieving those lists is worth doing
deliberately, from a machine that can reach the reports.
