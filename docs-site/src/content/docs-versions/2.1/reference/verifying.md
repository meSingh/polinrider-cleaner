---
title: Verifying this repository
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
gh attestation verify polinrider-v2.1.0-macos-arm64.tar.gz --repo meSingh/polinrider-cleaner
```

The Homebrew formula and the Scoop manifest carry the checksums from that same
`SHA256SUMS`, so a package manager install is checked against it too.

## Verify the commits

```bash
git log --show-signature -5
```

## The honest part

None of this proves the tool is correct, only that it is the code this
repository published. The source is deliberately readable for that reason: the
binary is one Rust crate with no dependencies, the indicator set is plain text
files, and every behaviour is pinned by a case in the conformance corpus.
