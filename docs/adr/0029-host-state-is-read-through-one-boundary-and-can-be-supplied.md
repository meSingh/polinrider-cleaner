# 0029. Host state is read through one boundary, and can be supplied

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-03 |

## Context

A third of the machine check does not read the disk it was pointed at. It reads
the machine it is running on: the process table, open sockets, the user
crontab, the global git configuration, the system's persistence directories.

The conformance corpus (ADR-0026) could not cover any of it. A result that
depends on what happens to be running is not a specification, so every case ran
with `--fs-only` and the corpus said, in its own README, that the live checks
"need their own approach". Until this record the Rust engine did not have them
at all: seven sections printed `skipped`.

So the part of the scanner that looks for a running implant had no test of any
kind, in either implementation, for as long as it has existed. That is the
failure this project fears most, a check that silently stops matching, and it
had already happened. Building the cases for this record found it:

**The implant process check has never matched on Linux.** The kernel keeps 15
bytes of a process name. The implant's name is 17. Run under its full name in
the sandbox, it appears in `ps` as the first 15 bytes, and the shell compares
whole names. Nothing failed, nothing warned, and the section printed
`no second-stage implant found` on every Linux machine it ever ran on.

## Decision

Everything the scanner asks of the machine, rather than of a file under a root
or the home directory it was given, goes through one trait, `Host`, in
`src/host.rs`. Nothing outside that module runs a command.

**Two implementations.** `LiveHost` asks the running machine, with the same
tools the shell used (`ps`, `ss` or `netstat`, `lsof`, `crontab`, `git`).
`Snapshot` holds the answers as data.

**The boundary carries `Probe`, not bare values.** A probe is `Read`, `NoTool`
or `Failed`. "The process table was empty" and "the process table could not be
read" are different answers, and each check has to say which it got. A probe
that failed is a `[review]` line and never an `[ok]`.

**`--host-state DIR` loads a `Snapshot` from files**, one per question:
`platform`, `processes`, `connections`, `crontab`, `git-config` and `root/`,
which stands in for `/`. An empty file is the empty answer. A file that is absent
means the question was not answered, and the scan reports that. `<name>.absent`
records that the tool was not installed where the state came from. The report
header says `supplied from DIR, NOT read from this machine`, and advice that
only makes sense on the machine itself, such as `kill -9` with a pid, is not
printed for supplied state.

**Host cases join the corpus.** A case with a `host` key describes a machine as
well as a tree. Twenty of them, each with a `why`.

**Where the port differs from the shell, it is deliberate and a case argues for
it.** Reading the shell closely enough to port it turned up eight places where
it was wrong in one of two directions.

It reported compromise where there was none, each of which breaks the rule that
a false `INFECTED` is worse than a missed `review`:

1. Any npm registry other than npmjs.org was a confirmed hit. Every company
   with an internal registry was told to rebuild the machine. Now `[review]`,
   and a hit only when the registry is the campaign's own host.
2. `curl ... | shasum` in a startup file matched as piping into `sh`, because
   the pattern did not ask what followed `sh`. A commented-out line matched
   too.
3. A campaign address matched as a fixed substring, so a connection to any
   address containing it was a live connection to the campaign.

It missed, or stayed silent about, what it should have reported:

4. The Linux process name, above.
5. Only sockets held by node and Electron were examined. The second stage is a
   native binary under its own name; its connection to the controller was
   never looked at.
6. A user crontab naming the campaign's controller was `[review]`, while the
   same content in a `cron.d` file or a unit was a hit.
7. A tool that failed was treated as an empty answer.
8. `npm config get ignore-scripts` executed npm, on a machine suspected of an
   npm supply-chain compromise, to ask what `~/.npmrc` already says. It is
   read from the file now.

Two smaller ones in the same spirit: a pasted cleanup command quotes the file
name properly, since whoever planted the file chose its name, and every line of
output is stripped of control characters on its way out, which the Rust engine
had not been doing at all.

## Consequences

Better: the untested third of the scanner is now the most heavily specified
part of it, and a check that stops matching fails a case.

Worse, and worth stating plainly:

- **The corpus no longer proves the two implementations agree, for these
  checks.** The shell cannot be handed a machine that does not exist, so the
  host cases skip under it, visibly and counted. For the filesystem checks the
  port is finished when both answer alike. For the host checks it is finished
  when the Rust engine answers the cases, and the cases were written by the
  same person, in the same sitting, as the code. ADR-0026 warned that a corpus
  can encode a wrong answer; here there is no second implementation to
  disagree with it.
- **Eight differences is eight chances to be wrong.** Each is a judgement that
  the shell was mistaken. If one of them is the shell being right for a reason
  not written down, the case now pins the mistake.
- **`LiveHost` is the part still not covered.** The boundary moves the
  untestable code into a thin layer; it does not remove it. Parsing real `ss`
  and `lsof` output, and the exit statuses of real `crontab` and `git`, are
  tested by one unit test and by running the binary in the sandbox, which is
  Linux and has neither `ss` nor `crontab`. The macOS path of `LiveHost` has
  never been run.
- **`--host-state` produces a report about a machine that is not this one.**
  The header says so, but a report is text and a header can be cut off. It
  also means a result can be manufactured. That was always true of a text
  report; the flag makes it a feature.
- **A partial capture always exits 1.** Unanswered questions are review items
  by design, so state captured without, say, a connection list can never come
  back clean. That is the right answer and it will be reported as a bug.
- **The state directory is a format, and formats are promises.** It is small
  and line-based, and it now has to stay readable by every later version.
- **Windows is refused, not handled.** Without `--fs-only` the binary exits 3
  on a platform whose live checks are not built. Honest, and no help to anyone
  on Windows until that track is ported.
- **Not everything the shell does here is ported.** The "Credential surface"
  inventory, the `stop it first` advice under an implant path, and the review
  of extensions that reference campaign infrastructure are still shell only.

## Related

- ADR-0026, 2.0.0 is one binary built against a conformance corpus: said the
  live checks would need injectable state, and that a corpus pins bugs as
  faithfully as it pins behaviour.
- ADR-0027, development and testing happen in a container: where the process
  name was observed being cut, rather than assumed.
- ADR-0002, exit code 3 means the scan could not run: the same rule, applied
  to one probe instead of the whole scan. A question that could not be asked
  is not an answer.
