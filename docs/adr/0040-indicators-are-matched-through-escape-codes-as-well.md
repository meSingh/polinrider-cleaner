# 0040. Indicators are matched through escape codes as well

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-13 |

## Context

The indicator review of 5 October
([the 2026-10-05 indicator review](https://mesingh.github.io/polinrider-cleaner/indicator-reviews/2026-10-05/),
technique 2) found that the `0xa322` loader is ordinary JavaScript with its
characters written as `\uXXXX` escapes. 18 of the 35 repositories SafeDep
confirmed carry it. Node decodes the escapes before it runs anything, so the
loader does what the plain one does, but the file does not contain the
indicator strings. `grep -F` against `ioc/` finds nothing in it, and neither did
anything in this repository. The better half of `strong.txt` (the `global.i`
assignments, the wallets, the XOR keys) cannot match that family whatever case
or spelling it is carried in.

Nothing in `ioc/` can fix that. The problem is not which string to look for but
that the file does not hold it. The README and `AGENTS.md` said so, and told
people to read the review before trusting a clean result on that family.

## Decision

**Every confirmed-indicator match also reads a decoded copy.** `has_strong` and
`has_bad_package` match the text as written first. If it holds a `\u` or a
`\x`, a copy is made with `\uXXXX` (surrogate pairs joined), `\u{X...}` and
`\xNN` replaced by the character, and that is matched too. Anything that is not
a valid escape is copied as it is. A backslash in front of an escape does not
stop it being decoded: a loader nested one string deep, a script inside a JSON
string, is written that way, and the copy is only ever matched. It is never
shown, written back or cut from.

Every content check goes through those two functions, so all of them pick it
up: `tasks.json`, manifests, git hooks, editor extensions, build configs and the
host checks. A `tasks.json` whose `folderOpen` is written in escapes is read as
one too, because the editor reads it that way.

**`clean` refuses an indicator it can only see decoded.** It cuts at a byte
offset, and an indicator that exists only in the decoded copy has none: a cut
worked out on the copy would be applied to bytes of a different length. The
file is a confirmed hit, not strippable, with the reason "the indicator is
written in escape codes, so the payload cannot be located in the file to cut".
It is never a `None`, which would read as nothing in the file. The check that
nothing is left after a cut reads the kept part decoded as well, so an escaped
copy above the padding stops a cut that would have left it behind.

**On GitHub, a second pass after the fixed-string one.** `git grep -F` stays.
`git grep -E` then lists the text files that hold an escape at all, those not
already found are read in one `git cat-file --batch`, and each is matched
decoded. The history pass behind erase already reads every blob and now matches
the decoded copy too. remove asks the same strip code for the cut, so an
escaped-only config is taken out whole there, as any config that cannot be cut
safely already is.

## Consequences

Better: the largest loader family in the 5 October analysis is caught by every
check that reads content, on the machine, in a folder and on GitHub, with the
indicator set as it is. The indicator files do not need a second, escaped copy
of anything.

Worse:

- **A file with escapes is read twice.** Text without a `\u` or `\x` costs one
  extra pass looking for one and no copy. Text with them, which is most minified
  bundles and most translation files, is decoded in full. The work is linear,
  but an extension directory full of bundles is now slower to check.
- **On GitHub, candidate files are read out of git.** `\xNN` is common, so on a
  large repository the candidate list can be long, and each one is read whole.
  It is one batch per branch, not one process per file, and it is paid on every
  branch of every repository.
- **`clean` cannot fix this family.** It confirms it and says why it will not
  cut. The operator is told to re-clone once the remote is clean, which is the
  advice for every refusal. Undoing an escaped payload by hand is possible and
  is not something this tool guesses at.
- **Decoding too much is a choice.** `"\\u0041"` is a backslash and five letters
  in JavaScript, and is an `A` here. A false positive needs an indicator to
  appear once decoded, which in practice means somebody wrote one in escapes,
  so this leans toward catching more.
- **Other encodings are still missed.** Base64, `String.fromCharCode` lists,
  string concatenation, octal and HTML entities all keep the indicator out of
  the bytes. This covers the one the field showed, not the general problem.
- **The weak list is not decoded.** It is review-only, and decoding it would
  add review lines to every bundle that happens to escape a common word.

## Alternatives rejected

- **A JavaScript tokenizer**, decoding exactly the escapes JavaScript would and
  nothing else. Correct about `\\u`, and a parser to maintain for a language
  that is not the only one these files are in (JSON, shell in a hook). The
  matcher does not need to be right about what a string means, only about
  whether an indicator is in it.
- **A regex tier for the shape of the loader**, as the review proposed for
  `_$_[0-9a-f]{4}=\(function`. It catches the family by how it looks rather
  than what it says, and a shape rule is one rename away from silence. It may
  still be worth having. It is not a substitute for reading what the file says.
- **Escaped copies of each indicator in `ioc/`.** Every indicator would need
  every mix of escaped and plain characters, which is not a list, and the
  weekly review would have to keep them in step.

## Related

- ADR-0031, why `clean` only cuts the one shape it recognises.
- ADR-0036, the GitHub check this adds a pass to.
- ADR-0039, why every new conformance case has to argue for itself.
