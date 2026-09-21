# 0028. The documentation site is Astro Starlight, on a Node toolchain

| | |
|---|---|
| **Status** | Accepted |
| **Date** | 2026-09-21 |
| **Supersedes** | the mdBook choice recorded in [ADR-0026](./0026-2-0-0-is-one-binary-built-against-a-conformance-corpus.md) |

## Context

ADR-0026 chose mdBook and ruled out anything on a Node toolchain, on the
grounds that a Node supply-chain cleanup tool should not carry npm
dependencies, even to build its documentation.

That constraint was held and it cost what it was always going to cost. The
maintainer's assessment of the result, after two rounds of styling: it still
does not look good, and a documentation site that looks unmaintained
undermines a tool whose entire pitch is that you can trust it enough to run it
on a compromised machine.

The framework survey behind ADR-0026 had already said this without it being
heard. No security tool documents with mdBook. They use Mintlify, GitBook,
VitePress, Docusaurus and Hugo. mdBook's default look reads as "a Rust
project's book", and the theming needed to escape that is work that competes
with building the tool.

## Decision

Astro Starlight, with the [Lucode](https://github.com/lucas-labs/lucode-starlight-theme)
theme, a shadcn/ui-styled Starlight theme. MIT licensed, though the author
declares it in `package.json` and has committed no `LICENSE` file.

Starlight over the alternatives: Docusaurus is more widely used but its default
appearance needs the same work mdBook did; Mintlify is the most polished and is
hosted, which hands the availability of this project's documentation to a third
party; docs.page is free and zero-effort but small, and has the same hosting
concern. Starlight ships almost no JavaScript, has the best default appearance
of the self-hosted options, and deploys to GitHub Pages exactly as mdBook did.

The Node dependency is accepted, with mitigations rather than pretending the
concern was wrong:

- `package-lock.json` is committed, and CI installs with `npm ci`, which fails
  rather than silently resolving a different tree.
- Direct dependencies are pinned to exact versions.
- `node_modules/` is a build-time artifact of the documentation only. It is
  never shipped, never required to run the tool, and the scanner prunes it.
- The theme is MIT and small enough to vendor if it is ever abandoned.

## Consequences

Better: a site that looks like somebody maintains it, which for this project is
a functional requirement rather than decoration. Search, asides, code blocks,
and mobile layout all work without being built by hand.

Worse, and worth stating plainly:

- **This repository now contains npm dependencies.** 230 packages, transitively,
  to render a documentation site for a tool that cleans up after an npm
  supply-chain worm. The mitigations above reduce that surface; they do not
  remove it, and the irony is real rather than rhetorical.
- **The build is no longer a single static binary.** mdBook was one Rust
  executable with no runtime. This needs Node, and a contributor who only wants
  to fix a typo now needs it too, unless they use the edit link on the page.
- **The theme has 32 stars and one maintainer.** That is a bus factor. It is
  MIT, and vendoring it is the exit if it stops being maintained.
- **The tool itself must never acquire a runtime dependency on any of this.**
  The scanner still has to run on a bare machine mid-incident with nothing
  installed. Documentation tooling and tool dependencies are separate, and the
  first must not leak into the second.

## Related

- ADR-0026, which chose mdBook and is superseded on this point only. Everything
  else it decided about 2.0.0 stands.
