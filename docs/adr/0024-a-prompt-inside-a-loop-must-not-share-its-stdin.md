# 0024. A prompt inside a loop must not share its stdin

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-06 |

## Context

"Clean every affected repository" looped over the list of repositories the usual
way:

```bash
while read -r repo; do
  clean_one ... "$repo"       # asks "Apply that to $repo?"
done < "$repos"
```

`done < "$repos"` puts the file on the loop's stdin, and `clean_one` inherits
it. The confirmation therefore read **the next repository name** as its answer.

Two things followed, and only the first was visible: the answer was never the
operator's, so every repository reported "left alone"; and the line consumed as
an answer was never processed, so **every other repository was skipped without
being scanned**. With five repositories, three were touched.

## Decision

The list is read into an array first, and the work loop iterates the array. The
loop body then inherits the terminal, as it should.

Confirming once per repository is also tedious across dozens, so the prompt
takes four answers: this one, skip it, all the rest without asking, or stop.

## Consequences

Anything interactive inside a loop has to be checked for this, and it is not
visible when reading the code: the loop looks ordinary and the failure looks
like the operator answering no.

The self-test asserts the shape rather than the behaviour, which is weaker but
cheap: the only loop fed by the repository list is a one-liner that builds the
array, so nothing that reads from the operator can be inside it.

`git` does the same thing. An earlier version of a helper that cloned inside a
`while read` loop stopped after one repository for exactly this reason, fixed
there with `</dev/null`. That fix does not work here, because the prompt needs
real stdin.
