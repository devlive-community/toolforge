#!/usr/bin/env bash
# Read-only GitHub history download. The signed .tfp bytes are never repacked.
set -euo pipefail
umask 077
out=${1:-dist/plugins}
repo=${GITHUB_REPOSITORY:?Set GITHUB_REPOSITORY to owner/repository}
tag=${RELEASE_TAG:-plugins}
[[ $repo =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ && $tag == plugins ]] || { echo 'Invalid repository or release tag' >&2; exit 1; }
mkdir -p "$out/previous"
[[ -z $(find "$out/previous" -mindepth 1 -print -quit) ]] || { echo 'Historical package directory must be empty' >&2; exit 1; }
# Verify repository read access first: do not interpret an inaccessible private repo as an empty release.
gh api "repos/$repo" --jq '.id' >/dev/null
scratch=$(mktemp -d)
errors=$scratch/error
trap 'rm -rf "$scratch"' EXIT
if ! gh api "repos/$repo/releases/tags/$tag" > "$out/previous-release.json" 2> "$errors"; then
  if grep -q '(HTTP 404)' "$errors"; then
    printf '{"id":null,"assets":[]}\n' > "$out/previous-release.json"
    echo 'No plugins release exists; bootstrapping an empty inventory'
  else
    echo 'Cannot read plugin release history; refusing to publish an incomplete inventory' >&2
    exit 1
  fi
fi
release_id=$(jq -r '.id // empty' "$out/previous-release.json")
if [[ -n $release_id ]]; then
  [[ $release_id =~ ^[0-9]+$ ]] || { echo 'Invalid release identifier' >&2; exit 1; }
  # Enumerate pages explicitly; a large history must never be truncated to one assets page.
  gh api --paginate --slurp "repos/$repo/releases/$release_id/assets" > "$scratch/pages.json"
  jq 'flatten' "$scratch/pages.json" > "$scratch/assets.json"
  jq --slurpfile assets "$scratch/assets.json" '.assets = $assets[0]' "$out/previous-release.json" > "$scratch/release.json"
  cp "$scratch/release.json" "$out/previous-release.json"
fi
jq -e '(.assets | type == "array") and all(.assets[]; (.id | type == "number") and (.name | type == "string"))' "$out/previous-release.json" >/dev/null
while IFS=$'\t' read -r asset_id name digest; do
  [[ $asset_id =~ ^[0-9]+$ && $name =~ ^[a-z0-9][a-z0-9.+-]*\.tfp$ ]] || { echo 'Unsafe historical asset metadata' >&2; exit 1; }
  gh api -H 'Accept: application/octet-stream' "repos/$repo/releases/assets/$asset_id" > "$out/previous/$name"
  if [[ $digest == sha256:* ]]; then
    expected=${digest#sha256:}
    [[ $expected =~ ^[a-f0-9]{64}$ ]] || { echo 'Invalid GitHub asset checksum' >&2; exit 1; }
    printf '%s  %s\n' "$expected" "$out/previous/$name" | sha256sum --check --status
  fi
done < <(jq -r '.assets[] | select(.name | endswith(".tfp")) | [.id,.name,(.digest // "")] | @tsv' "$out/previous-release.json")
echo 'Historical assets downloaded; official signature verification is required before publication'
