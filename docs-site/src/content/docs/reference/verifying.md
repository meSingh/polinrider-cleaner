---
title: Verifying this repository
---

You are being asked to run unknown code on a machine you may believe is
compromised. That deserves more than a promise.

| Signal | What it proves |
|---|---|
| GPG-signed commits | Every commit was made by the key holder. This is the direct counter to the campaign's own backdated-amend technique |
| Attested releases | Each release carries a sigstore `.intoto.jsonl` provenance attestation, so you can verify the tarball came from this repository's CI and not from someone's laptop |
| `SHA256SUMS` on every release | The tarball you downloaded is the tarball that was built |
| OpenSSF Scorecard | 18 automated checks on the repository's security posture, published so the badge resolves |
| CodeQL and Semgrep in CI | Static analysis on every push |

## Verify a release

```bash
gh attestation verify polinrider-cleaner-v1.0.9.tar.gz --repo meSingh/polinrider-cleaner
shasum -a 256 -c SHA256SUMS
```

## Verify the commits

```bash
git log --show-signature -5
```

## The honest part

None of this proves the tool is correct, only that it is the code this
repository published. The source is deliberately readable for that reason: the
binary is one Rust crate with no dependencies, the indicator set is plain text
files, every behaviour is pinned by a case in the conformance corpus, and every
design
decision that could have gone the other way is
[recorded with its cost](/project/decisions/).
