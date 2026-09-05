## Verifying this release

Every artifact here is checksummed and carries a build-provenance attestation.
Verify before you run any of it. This is a tool for recovering from tampered
code, so take it at its word about nothing.

```bash
# 1. the checksum matches
sha256sum -c SHA256SUMS

# 2. the archive really was built by this repository's CI, at this tag
gh attestation verify polinrider-cleaner-__TAG__.tar.gz \
  --repo meSingh/polinrider-cleaner
```

The tag itself is GPG-signed:

```bash
git tag -v __TAG__
```

## What is in it

Shell and PowerShell only. No Node, no Python, nothing to install beyond `git`,
`gh` and `jq`. Every destructive step is a dry run first.

See the [README](https://github.com/meSingh/polinrider-cleaner#readme) to pick
the part you need, and [AGENTS.md](https://github.com/meSingh/polinrider-cleaner/blob/main/AGENTS.md)
if you are handing this to an AI agent.
