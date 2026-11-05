---
title: Verify polinrider before you run it
description: "How to check that polinrider came from this repository: signed commits, attested releases and SHA256SUMS on every release."
---

You are being asked to run unknown code on a machine you may believe is
compromised. That deserves more than a promise.

| Signal | What it proves |
|---|---|
| GPG-signed commits | Every commit was made by the key holder. This is the direct counter to the campaign's own backdated-amend technique |
| Attested releases | Each release carries a sigstore `.intoto.jsonl` provenance attestation covering every archive, the Windows zip and the `.deb` packages, so you can verify the file came from this repository's CI and not from someone's laptop |
| `SHA256SUMS` on every release | The file you downloaded is the file that was built |
| OpenSSF Scorecard | 18 automated checks on the repository's security posture, published so the badge resolves |
| CodeQL and Semgrep in CI | Static analysis on every push |

## Verify a release

The same two checks for any asset: an archive, the Windows zip or a `.deb`.

```bash
sha256sum -c SHA256SUMS --ignore-missing
gh attestation verify polinrider-v3.0.0-macos-arm64.tar.gz --repo meSingh/polinrider-scanner
```

The Homebrew formula and the Scoop manifest carry the checksums from that same
`SHA256SUMS`, so a package manager install is checked against it too.

## Releases before 3.0.0

In 3.0.0 the repository was renamed from `meSingh/polinrider-cleaner` to
`meSingh/polinrider-scanner`. Releases up to 2.3.0 were built and attested
under the old name. Their `SHA256SUMS` and their signed tags check exactly as
before. For their build attestation, `--repo meSingh/polinrider-scanner` does
not work, but this one command checks a release from any version:

```bash
gh attestation verify polinrider-v2.3.0-macos-arm64.tar.gz \
  --owner meSingh \
  --cert-identity-regex '^https://github\.com/meSingh/polinrider-(cleaner|scanner)/\.github/workflows/release\.yml@refs/tags/v'
```

The old name, the offline check and how to prove an attestation came from
this repository: [Verifying releases from before 3.0](/docs/3.0/reference/verifying-before-3/).

## Verify the commits

```bash
git log --show-signature -5
```

## The honest part

None of this proves the tool is correct, only that it is the code this
repository published. The source is deliberately readable for that reason: the
binary is one Rust crate with no dependencies, the indicator set is plain text
files, and every behaviour is pinned by a case in the conformance corpus.
