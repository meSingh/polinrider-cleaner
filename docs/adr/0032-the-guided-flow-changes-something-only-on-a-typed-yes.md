# 0032. The guided flow changes something only on a typed yes

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-03 |

## Context

ADR-0026 decided that running `polinrider` with no arguments enters a guided
flow: triage, machine check, credential rotation, remote cleanup, verification
and prevention, in one session, with no second command and no second terminal.
The reason was that the maintainer could not tell which of seventeen scripts
to run while cleaning a backup drive.

1.x has a guided flow in `polinrider.sh`, and three of the records in this
directory are about its prompts going wrong. A blank line read as "quit"
(ADR-0021, then ADR-0022). A prompt inside a loop read the loop's own data as
the operator's answer and reported repositories as "left alone" that were
never scanned (ADR-0024), which ADR-0026 calls the worst failure of the whole
project. A flow that asks questions is where this tool has been most wrong.

The Rust engine had `check` and `clean` and no flow. It also has no GitHub
track at all: scanning an organization, restoring a branch and cleaning a
remote are still shell.

## Decision

`polinrider` with no arguments, or `polinrider guide`, runs one session of six
steps: what to check, where to look, scan, contain, credentials and remotes,
prevent.

**The scan is a dry run that shows what containing would do.** The same lines
`clean` prints: what would be moved, what would be stripped, the last line
kept and the start of what goes.

**Three rules for every prompt.**

- Only `yes` changes anything. Not Enter, not a default, not anything that
  merely is not `no`. Anything else asks again.
- Only `q` leaves. A blank line never quits and never chooses.
- Input that ends, stops. If stdin closes, the session ends where it is with
  nothing further changed, and says so. The end of input is not an empty line
  and is never an answer.

**It offers only what it can do.** If nothing found can be moved or stripped,
a running process for instance, it says so and does not put a question that
has no yes.

**It refuses to be pre-answered.** `guide --apply` is an error, and so is a
directory on the command line. Somebody who wants no prompts wants `check` or
`clean`.

**The exit code is the worst thing found in the session.** A session that
finds a payload, strips it and verifies the files clean exits 2. A session
left before any scan exits 3, not 0: nothing was scanned.

**All reading and printing goes through one trait, `Console`.** A test drives
a whole session from a list of answers, and the conformance corpus does the
same through the binary with a case's `stdin`.

**The remote step is honest about being absent.** Step 5 tells the operator to
rotate credentials from a different machine and says in so many words that
this build does not scan or clean GitHub, and that the released tool does.

## Consequences

Better: the prompt failures of 1.x are each pinned by a case, and a flow that
could only be tested by hand is now tested by the corpus.

Worse, and worth stating plainly:

- **This guided flow does less than the one it replaces.** 1.x walks GitHub:
  it scans an organization or an account, offers a restore where push events
  survive and cleans where they do not. 2.0 stops at the machine and tells you
  the rest is yours. ADR-0026's "no second command" is not true yet. Porting
  the GitHub tracks is not on the list of remaining work in the handover at
  all, and it is the largest piece left before 2.0 can replace 1.x.
- **Exit 2 after a successful cleanup will be reported as a bug.** It is the
  right answer for a script, which must not be told nothing happened. It is a
  surprising answer for a person who just watched the second scan come back
  clean. The last line of the session explains it.
- **A typed yes is only as deliberate as stdin.** `printf 'yes' | polinrider`
  works, and has to, because that is how the tests drive it. The rule stops an
  accidental keypress, not a determined script.
- **No colour, no menus.** 1.x has a presentation layer in `ui/`. This is
  plain lines. It is easier to audit and worse to look at mid-incident, when
  the one line that matters needs to stand out.
- **The folder guesses are guesses.** "This computer" offers the usual code
  directories that exist under the home directory. Code kept anywhere else is
  not scanned unless the operator types it, and the flow says the scan is only
  as good as the directories it is given, which people do not read.
- **Typing paths is slow and unforgiving.** One directory per line, no
  completion. A path with a typo is rejected and asked for again, which is
  correct and irritating.

## Related

- ADR-0021 and ADR-0022, what leaves a prompt: the rule here is the one those
  two arrived at, written down before the code this time.
- ADR-0024, a prompt inside a loop must not share its stdin: the failure the
  end-of-input rule exists to make impossible.
- ADR-0026, 2.0.0 is one binary: where the guided flow was decided, and whose
  "no second command" this does not yet deliver.
- ADR-0031, clean strips an appended payload in place: what "contain" does
  when the answer is yes.
