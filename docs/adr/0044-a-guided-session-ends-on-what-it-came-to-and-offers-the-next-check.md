# 0044. End a guided session on what it came to, and offer the next check

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-24 |

## Context

Every check in the guided flow ended the same way: step 4's list, the report
path, `To check again: polinrider`, and the shell prompt. On a real GitHub
run, a restore that worked ended there, and it read as if the program had
stopped early. Nothing said the session was over, and the first thing on the
list ("check the computers") was a command to remember, not a word to type.

`everything` already chained this computer and then GitHub, but only in that
order and only when chosen at step 1.

## Decision

Step 4 opens with one sentence that says where things stand ("GitHub is clean
again.", "Nothing found on this computer.") and a few labelled lines of what
this run did: what was checked, what was found, fixed, verified on GitHub,
contained or left. The list follows under "Four things are still yours, in
order:".

Under the list, the check that usually comes next is offered as a word:
after GitHub, `computer`; after this computer or a folder, `organization`
and `account`; always `done`. Once this computer is done, GitHub stays on
offer, so `everything` is now this computer followed by the same offer.
Nothing is offered when nothing applies.

`done`, or `q` at that question, ends on a closing screen: "Finished.", each
check's sentence, how many steps are still the operator's, where the list is
saved, and one line on when to run it again. The session ends on a sentence,
not on a prompt.

A second check is weighed against the first with the existing rule: the
worse result is the exit code. A clean computer does not take back a
payload found on GitHub.

## Consequences

- The session can now run several checks, in any order the offers allow.
  The report holds all of them, and the exit code is the worst of them.
  A script that drove one check and expected the program to end after step
  4 now meets one more question; input that ends there stops as before.
- `q` at the offer is not a stop: it ends on the closing screen, because
  nothing is left to stop. Everywhere else `q` still means what ADR-0021
  says.
- The step count still says 4 of 4 for each check, so a second check shows
  steps 2 to 4 again. That is honest about each check, and slightly odd for
  the session as a whole.
- The closing screen counts the steps of the last list only. After two
  checks, the earlier list is in the report, not in the count.
- The `everything` choice at step 1 now does nothing that `computer` and
  the offer would not; it stays because it says the order out loud.

## Related

- ADR-0035: four calm screens answered in words. The offer is one more
  question on step 4, answered in words.
- ADR-0021: only `q` leaves a prompt.
- ADR-0037: the exit code reports what was found, even after it was fixed.
