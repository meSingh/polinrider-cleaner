---
title: Verifying releases from before 3.0
description: "Releases up to 2.3.0 were built when the repository was called polinrider-cleaner. How to check one of them is genuine now that it is polinrider-scanner."
---

In 3.0 the repository was renamed from `meSingh/polinrider-cleaner` to
`meSingh/polinrider-scanner`. Releases up to and including 2.3.0 were built
and attested under the old name, and their attestations record that name.
3.0.0 is the first release built under the new one.

## What the rename does not change

**The checksums.** Every release still carries its `SHA256SUMS`, and checking
a download against it works as before:

```bash
sha256sum -c SHA256SUMS --ignore-missing
```

**The signed tags.** Each release's tag is signed, and the signature belongs to
the tag, not to the repository's name:

```bash
git verify-tag v2.3.0
```

## The build attestation

### One command for every release

This checks a release from any version, before or after 3.0. It accepts an
attestation made by this project's release workflow, from a release tag,
under either name:

```bash
gh attestation verify polinrider-v2.3.0-macos-arm64.tar.gz \
  --owner meSingh \
  --cert-identity-regex '^https://github\.com/meSingh/polinrider-(cleaner|scanner)/\.github/workflows/release\.yml@refs/tags/v'
```

`--owner` limits the lookup to this account, and `--cert-identity-regex`
requires that the file was signed by `release.yml` in this repository, under
its old or its new name, while building a tag.

### With the old name

For a release from before 3.0, the command that worked before the rename
still works. GitHub redirects the old name to the renamed repository:

```bash
gh attestation verify polinrider-v2.3.0-macos-arm64.tar.gz --repo meSingh/polinrider-cleaner
```

The new name does not work for these releases. Their attestations record the
old name, so `--repo meSingh/polinrider-scanner` fails with
`Error: verifying with issuer "sigstore.dev"`. That error on a release from
before 3.0.0 means the wrong name was used, not that the file is fake.

### Offline

Every release ships its attestation as `polinrider-vX.Y.Z.intoto.jsonl`.
`--bundle` reads it from that file instead of asking GitHub:

```bash
gh attestation verify polinrider-v2.3.0-macos-arm64.tar.gz \
  --bundle polinrider-v2.3.0.intoto.jsonl \
  --repo meSingh/polinrider-cleaner
```

### Proof that it came from this repository

A name can change hands; a repository's ID does not change when it is
renamed. To be sure the attestation was made in this repository, and not in
some later repository given the old name, print the ID it records:

```bash
gh attestation verify polinrider-v2.3.0-macos-arm64.tar.gz \
  --owner meSingh \
  --cert-identity-regex '^https://github\.com/meSingh/polinrider-(cleaner|scanner)/\.github/workflows/release\.yml@refs/tags/v' \
  --format json --jq '.[].verificationResult.signature.certificate.sourceRepositoryIdentifier'
```

It must print `1358099890`, this repository's ID, for every attestation it
finds.

All of the above was tested on 4 November 2026 with `gh` 2.100.0, on 2.3.0 and
2.0.0, after the rename.

## Or use a current release

Unless you need a particular old version, download the latest release and
verify it the usual way ([Verifying this repository](/docs/reference/verifying/)).
