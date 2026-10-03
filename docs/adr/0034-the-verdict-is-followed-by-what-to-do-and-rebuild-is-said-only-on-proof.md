# 0034. The verdict is followed by what to do, and "rebuild" is said only on proof

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-10-03 |

## Context

The Rust engine ended a report at the verdict. The maintainer ran it, read
`VERDICT: COMPROMISED`, and asked the obvious question: what next? 1.x answers
it with four fixed lines under the box. The port had dropped them.

Those four lines also say the same thing whatever was found: disconnect,
rotate, rebuild, delete every clone. And the box says "This machine cannot be
trusted until it is rebuilt" for a scan of one folder that turned up a payload
in a cloned config file. That may be true. Nothing in the scan shows it.

## Decision

Every report ends with a short numbered block, **WHAT TO DO NEXT**, worked out
from what was found, with the exact command wherever there is one.

**Every `[HIT]` carries a kind**, and the kind is an argument to
`Finding::hit`, so a finding the block cannot account for does not compile.
The kinds are what can be done about it and what it proves: a config that
`clean` can strip or cannot, a file `--apply` moves, a package to remove by
hand, something outside the projects that `--apply` moves, something only a
person can edit, something running now.

**Commands are this run's, never a template.** The same directories and the
same scope flags, quoted so they paste. `clean` lines drop the flags `clean`
refuses. After an `--apply` the block does not say to apply again.

**"Rebuild" is said when something proves the payload ran here**: an implant,
a login item, a hook, an extension, a startup file, a process, a connection.
When the only findings are files inside projects, the box says the payload is
in your project files and the block says how to decide: the payload runs when
an infected project is built or opened in an editor, and if that happened, or
you are not sure, treat the machine as compromised and rebuild it.

A review-only result gets three lines on how to read a `[review]`, and a clean
one gets a single sentence. The guided flow has its own steps and does not
print the block.

## Consequences

Better: the report answers the question it raises, and the first step is a
command that can be pasted.

Worse:

- **Softer advice can be wrong in the dangerous direction.** 1.x told
  everybody to rebuild. This tells somebody with a payload in a cloned file to
  decide, and a stressed person may decide what is convenient. The wording
  puts "not sure" on the rebuild side, which is as far as text can push it.
- **"Ran here" is a judgement baked into an enum.** A git hook counts as
  proof and a `tasks.json` carrying an indicator does not, though opening the
  folder would have run it. Each of those calls could be argued the other way.
- **The commands assume the binary is called `polinrider` and is on the
  PATH.** Run as `./target/release/polinrider`, the printed line does not work
  as pasted.
- **More text under the worst verdict.** Seven steps at most, wrapped to 78
  columns, where there was a box. It is the part people most need and it is
  also more to read at the moment they can least read.

## Related

- ADR-0029, a false `INFECTED` is worse than a missed `review`: the same
  argument, applied to "rebuild".
- ADR-0031, clean: the command step two usually points at.
- ADR-0033, the banner and colour: command lines in the block are coloured
  as commands, by the same module.
