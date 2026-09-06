# ui/

Everything that draws. Nothing in this directory reads a repository, runs git,
or decides whether something is infected.

That separation is the point. If you are auditing this tool, you can skip this
directory entirely and read `lib/` and `ci/` on their own. And if you dislike
how it looks, you can change it here without touching anything that matters.

| File | Holds | Lines to read |
|---|---|---|
| `theme.sh` | colours, symbols, terminal capability detection | palette only, no logic |
| `render.sh` | the drawing functions | takes text, prints text |

## The layout rule

Indentation carries meaning, so output can be scanned rather than read:

```
Section                    a stage of the work, flush left, ruled
  ▸ Step                   something being done, 2 spaces
      ✓ result             an outcome, 6 spaces, marked and coloured
      dimmed git output    raw subprocess output, 6 spaces, grey
```

Anything a subprocess prints goes through `ui_stream` or `ui_findings`, so git
chatter can never sit at the same visual level as a finding. That was the
problem this directory exists to fix: timestamps, git output and confirmed hits
all printed flush left, in one colour, with no way to tell them apart.

## Colour carries meaning

Four colours, each bound once in `theme.sh`, so a finding cannot render as
anything else anywhere in the tool.

| Colour | Means |
|---|---|
| red | a confirmed indicator |
| yellow | needs a person to look at it |
| green | checked and clean |
| grey | inventory or progress, not a finding |

`ui_findings` dims anything it does not recognise. Unknown text never borrows
the colour of a finding.

## It turns itself off

No configuration needed, and no flag to remember:

| Condition | Effect |
|---|---|
| output is not a terminal | no colour, no cursor control, no progress bar |
| `NO_COLOR` set to anything | no colour ([no-color.org](https://no-color.org)) |
| `TERM=dumb` | no colour, no cursor control |
| locale is not UTF-8, or `PRC_ASCII=1` | ASCII symbols instead of box drawing |
| terminal narrower than 60 columns | treated as 60 |

Piping to a file or a CI log produces plain text with no escape sequences.

## No dependencies, on purpose

`gum`, `dialog` and `whiptail` are all better than this, and all of them are a
binary someone has to install. This tool exists to clean up a supply-chain
compromise; asking you to install a third-party binary so it can look nicer is
the wrong trade. `tput` is used for terminal width and nothing else.
