# False positives you will see

All of these were confirmed harmless during a real cleanup. Precision matters
more than reach here: a false `INFECTED` in a tool people run during an incident
costs everyone real time.

| What you see | Why | What to do |
|---|---|---|
| Your own scan workflow flagged `INFECTED` | It contains the indicator strings because it searches for them | The triage filter removes these. Add your own paths to its `BENIGN_RE` |
| `.vscode/settings.json` matched `folderOpen` | Only `.vscode/`**`tasks`**`.json` executes commands | Ignore. Verdict is `review`, never `INFECTED` |
| Every `.woff2` in a repo flagged as "not a font" | Git LFS stores a text pointer instead of the font, and a zero-byte placeholder has no magic bytes | Already handled: LFS pointers and empty files are skipped |
| A config file flagged for "content after module end" | Flat configs legitimately open with `export default [` on line 1 and run long | Already handled: fires only when the remainder also looks like a payload |
| A README or incident writeup flagged `INFECTED` | Documenting the campaign means naming its indicators | Already handled: `.md` is skipped. `--scan-docs` includes it |
| Rendered documentation (HTML, a built docs site) flagged `review` | Same reason, but the `.md` exemption keys off the extension, which the rendered output no longer has | Exclude your build output: `--exclude '(^\|/)site/'`. This repository excludes its own `docs-site/book/` |

Found a new one?
[Open an issue](https://github.com/meSingh/polinrider-cleaner/issues/new?template=false-positive.md).
A missed detection is [a different template](https://github.com/meSingh/polinrider-cleaner/issues/new?template=missed-detection.md),
and both are worth filing.
