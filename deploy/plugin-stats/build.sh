#!/usr/bin/env bash
# Build the plugin statistics service binary exactly like the deploy workflow.
# Usage: build.sh [--skip-tests] [output-dir]
# Produces <output-dir>/plugin-stats and <output-dir>/plugin-stats.sha256.
# The commit SHA is injected via -X main.buildSHA; deploy.sh readiness check
# compares it against the deployed commit, so omitting it breaks deployment.
set -euo pipefail
cd "$(dirname "$0")/../.." # repository root

skip_tests=false
out=build
while [[ $# -gt 0 ]]; do
  case $1 in
    --skip-tests) skip_tests=true ;;
    -h|--help) echo "Usage: build.sh [--skip-tests] [output-dir]"; exit 0 ;;
    *) out=$1 ;;
  esac
  shift
done

[[ -f apps/server/go.mod ]] || { echo 'apps/server/go.mod not found; run from the repository' >&2; exit 1; }
commit=$(git rev-parse HEAD)
# Keep the regex in a variable: bash 3.2 brace-expands {40} inside [[ =~ ]].
sha_re='^[a-f0-9]{40}$'
[[ $commit =~ $sha_re ]] || { echo 'Could not resolve a full commit SHA; build inside a git checkout' >&2; exit 1; }

cd apps/server
if [[ $skip_tests != true ]]; then
  test -z "$(gofmt -l .)" || { echo 'gofmt differences found; run gofmt first' >&2; exit 1; }
  go mod download
  go mod verify
  go vet ./...
  go test -race -count=1 ./...
fi
mkdir -p "$out"
# Keep every flag identical to .github/workflows/plugin-stats-deploy.yml.
CGO_ENABLED=0 GOOS=linux GOARCH=amd64 \
  go build -trimpath -ldflags "-s -w -X main.buildSHA=$commit" \
  -o "$out/plugin-stats" ./cmd/plugin-stats
sha256sum "$out/plugin-stats" > "$out/plugin-stats.sha256"
echo "Built $out/plugin-stats ($(cut -d ' ' -f 1 "$out/plugin-stats.sha256")) for commit $commit"
echo 'Deploy: ssh <deploy-target> "deploy '"$commit"' <checksum>" < '"$out/plugin-stats"
