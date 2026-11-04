---
title: Verifying releases from before 3.0
description: "Releases up to 2.3.0 were built when the repository was called polinrider-cleaner. How to check one of them is genuine now that it is polinrider-scanner."
# DRAFT. Not built until the rename has been tested. Keep the outcome that
# the test shows, delete the other, delete this comment and the draft line,
# and link this page from reference/verifying.md and the quick start.
draft: true
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

<!-- OUTCOME A: keep this section if the rename test shows that the new name
     verifies an old release. -->

**If the test shows the new name works:**

The same command as for a current release works for an old one:

```bash
gh attestation verify polinrider-v2.3.0-macos-arm64.tar.gz --repo meSingh/polinrider-scanner
```

<!-- OUTCOME B: keep this section if the new name fails for an old release. -->

**If the test shows the new name fails:**

The attestation of a release from before 3.0 names the old repository, so a
check against the new name does not match. Check it against the name it was
built under, using the attestation file that ships with the release:

```bash
gh attestation verify polinrider-v2.3.0-macos-arm64.tar.gz \
  --bundle polinrider-v2.3.0.intoto.jsonl \
  --owner meSingh \
  --signer-repo meSingh/polinrider-cleaner
```

`--bundle` reads the attestation from the file instead of asking GitHub,
`--owner` limits it to this account, and `--signer-repo` requires that the
release workflow that signed it ran in the repository under its old name.

## Or use a current release

Unless you need a particular old version, download the latest release and
verify it the usual way ([Verifying this repository](/docs/reference/verifying/)).
