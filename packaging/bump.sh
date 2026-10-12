#!/usr/bin/env bash
# bump.sh - open a pull request moving one package manifest to a new release.
#
#   GH_TOKEN=... packaging/bump.sh REPO FILE VERSION SHA256SUMS RENDERER
#
#   REPO        meSingh/homebrew-tap or meSingh/scoop-bucket
#   FILE        the manifest's path in REPO
#   VERSION     the version without the v
#   SHA256SUMS  the release's checksum file
#   RENDERER    packaging/formula.sh or packaging/scoop.sh
#
# Run by release.yml with PACKAGING_TOKEN. It never pushes to the default
# branch. It creates a branch, makes one commit on it and opens a pull request,
# all through GitHub's API, and the maintainer merges it.
#
# The commit is made with the GraphQL createCommitOnBranch mutation rather than
# with git. GitHub signs commits made that way with its own key, so the commit
# shows as Verified, authored by the token's owner with GitHub as the signer,
# exactly like an edit made in the web interface. A git commit made on the
# runner would be unsigned, or would need the maintainer's private key in a
# secret, and neither is acceptable here. The script checks the signature
# before it opens the pull request and stops if GitHub did not verify it.

set -euo pipefail

die() { echo "::error::bump.sh: $*" >&2; exit 1; }

[[ $# -eq 5 ]] || die "usage: packaging/bump.sh REPO FILE VERSION SHA256SUMS RENDERER"
repo=$1 file=$2 version=$3 sums=$4 renderer=$5
[[ -n "${GH_TOKEN:-}" ]] || die "GH_TOKEN is not set"
command -v jq >/dev/null || die "jq is needed"

owner=${repo%%/*}
branch="polinrider-cleaner-${version}"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

base=$(gh api "repos/${repo}" --jq .default_branch)
head=$(gh api "repos/${repo}/git/ref/heads/${base}" --jq .object.sha)

gh api "repos/${repo}/contents/${file}?ref=${head}" \
  -H 'Accept: application/vnd.github.raw' > "${work}/current"
"$renderer" "$version" "$sums" < "${work}/current" > "${work}/new"

if cmp -s "${work}/current" "${work}/new"; then
  echo "::notice title=${repo}::${file} is already at ${version}. Nothing to do."
  exit 0
fi

open=$(gh api "repos/${repo}/pulls?state=open&head=${owner}:${branch}" --jq '.[0].html_url // empty')
if [[ -n "$open" ]]; then
  echo "::notice title=${repo}::A pull request for ${version} is already open: ${open}"
  exit 0
fi
if gh api "repos/${repo}/git/ref/heads/${branch}" >/dev/null 2>&1; then
  die "${repo} already has a branch ${branch} with no open pull request. Delete it or open the pull request by hand, then re-run this job."
fi

echo "== ${repo}: ${file}"
diff -u "${work}/current" "${work}/new" || true

gh api -X POST "repos/${repo}/git/refs" -f "ref=refs/heads/${branch}" -f "sha=${head}" >/dev/null

headline="polinrider-cleaner ${version}"
body="Moves the package to v${version}. Every checksum is taken from the release's SHA256SUMS, which is covered by the release's build-provenance attestation."
jq -n \
  --arg query 'mutation($input: CreateCommitOnBranchInput!) { createCommitOnBranch(input: $input) { commit { oid } } }' \
  --arg repo "$repo" --arg branch "$branch" --arg head "$head" \
  --arg headline "$headline" --arg body "$body" \
  --arg path "$file" --arg contents "$(base64 < "${work}/new" | tr -d '\n')" \
  '{query: $query, variables: {input: {
      branch: {repositoryNameWithOwner: $repo, branchName: $branch},
      expectedHeadOid: $head,
      message: {headline: $headline, body: $body},
      fileChanges: {additions: [{path: $path, contents: $contents}]}
    }}}' > "${work}/mutation.json"

oid=$(gh api graphql --input "${work}/mutation.json" --jq .data.createCommitOnBranch.commit.oid)
[[ "$oid" =~ ^[0-9a-f]{40}$ ]] || die "the commit was not created on ${repo}"

verified=$(gh api "repos/${repo}/commits/${oid}" --jq '.commit.verification | "\(.verified) \(.reason)"')
echo "commit ${oid}: verification ${verified}"
[[ "$verified" == "true "* ]] \
  || die "GitHub did not verify commit ${oid} on ${repo} (${verified}). The branch ${branch} is left for inspection and no pull request was opened."

release="https://github.com/meSingh/polinrider-cleaner/releases/tag/v${version}"
case "$(basename "$renderer")" in
  scoop.sh) asset="polinrider-v${version}-windows-x86_64.zip" ;;
  *) asset="polinrider-v${version}-macos-arm64.tar.gz" ;;
esac
jq -n --arg title "$headline" --arg head "$branch" --arg base "$base" --arg body "$(cat <<EOF
Moves \`${file}\` to [v${version}](${release}).

Every URL points at an asset of that release, and every checksum is the one in its \`SHA256SUMS\`. To check one by hand:

\`\`\`bash
gh release download v${version} --repo meSingh/polinrider-cleaner --pattern SHA256SUMS --pattern ${asset}
sha256sum -c SHA256SUMS --ignore-missing
gh attestation verify ${asset} --repo meSingh/polinrider-cleaner
\`\`\`

Opened by the release workflow of meSingh/polinrider-cleaner. The commit was made through GitHub's API and is signed by GitHub.
EOF
)" '{title: $title, head: $head, base: $base, body: $body}' > "${work}/pull.json"

url=$(gh api -X POST "repos/${repo}/pulls" --input "${work}/pull.json" --jq .html_url)
echo "::notice title=${repo}::Opened ${url}"
