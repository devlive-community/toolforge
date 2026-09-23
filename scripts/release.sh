#!/usr/bin/env bash
#
# ToolForge 发布脚本
#
# 流程：预检 → 运行检查 → （需要时）更新版本号并提交 → 创建附注标签（内容为该版本的提交历史）
#      → 推送标签触发 GitHub Actions release 工作流 → （可选）等待工作流完成
#      → 把版本号更新为下一个开发版本并提交。
#
# 兼容 macOS 自带的 bash 3.2。

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

REMOTE="origin"
DRY_RUN=0
ASSUME_YES=0
SKIP_CHECKS=0
WAIT=0
PUSH_BRANCH=0
NOTES_ONLY=0
NEXT="patch"
TARGET=""
WORKFLOW="release.yml"

# ---------------------------------------------------------------- 输出

if [ -t 1 ]; then
  C_RESET=$'\033[0m' C_BOLD=$'\033[1m' C_DIM=$'\033[2m'
  C_RED=$'\033[31m' C_GREEN=$'\033[32m' C_YELLOW=$'\033[33m' C_CYAN=$'\033[36m'
else
  C_RESET='' C_BOLD='' C_DIM='' C_RED='' C_GREEN='' C_YELLOW='' C_CYAN=''
fi

step() { printf '\n%s==>%s %s%s%s\n' "$C_CYAN" "$C_RESET" "$C_BOLD" "$*" "$C_RESET"; }
info() { printf '    %s\n' "$*"; }
ok() { printf '%s✓%s %s\n' "$C_GREEN" "$C_RESET" "$*"; }
warn() { printf '%s!%s %s\n' "$C_YELLOW" "$C_RESET" "$*" >&2; }
die() {
  printf '%serror:%s %s\n' "$C_RED" "$C_RESET" "$*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Usage: scripts/release.sh [options] [version | patch | minor | major]

Publish a ToolForge release. A v<version> tag is created whose message is the
version's commit history; pushing it triggers the GitHub release workflow.
Afterwards the project version is bumped to the next development version and
committed.

Version:
  (none)                  Release the current version (apps/desktop/package.json)
  1.2.0, 1.2.0-beta.1     Release exactly this version
  patch | minor | major   Bump the current version, then release it

Options:
  -n, --dry-run           Show the plan and release notes, change nothing
  -y, --yes               Do not ask for confirmation
      --next <v|kind>     Next development version: patch (default), minor, major,
                          an exact version, or "none" to keep the released version
      --wait              Wait for the GitHub release workflow (needs gh); only bump
                          the version when it succeeds
      --push              Also push the current branch (release and bump commits)
      --skip-checks       Skip cargo xtask check (not recommended)
      --remote <name>     Git remote to push to (default: origin)
      --notes             Only print the release notes for the target version
  -h, --help              Show this help

Examples:
  scripts/release.sh                    # release the current version
  scripts/release.sh minor --wait       # bump minor, release, wait, then bump patch
  scripts/release.sh 1.0.0 --next none  # release 1.0.0 and keep the version as is
  scripts/release.sh --dry-run          # preview the plan and notes
EOF
}

# ---------------------------------------------------------------- 参数

while [ $# -gt 0 ]; do
  case "$1" in
    -n | --dry-run) DRY_RUN=1 ;;
    -y | --yes) ASSUME_YES=1 ;;
    --wait) WAIT=1 ;;
    --push) PUSH_BRANCH=1 ;;
    --skip-checks) SKIP_CHECKS=1 ;;
    --notes) NOTES_ONLY=1 ;;
    --next)
      [ $# -ge 2 ] || die "--next requires a value"
      NEXT="$2"
      shift
      ;;
    --next=*) NEXT="${1#*=}" ;;
    --remote)
      [ $# -ge 2 ] || die "--remote requires a value"
      REMOTE="$2"
      shift
      ;;
    --remote=*) REMOTE="${1#*=}" ;;
    -h | --help)
      usage
      exit 0
      ;;
    -*) die "unknown option: $1 (see --help)" ;;
    *)
      [ -z "$TARGET" ] || die "only one version may be given"
      TARGET="$1"
      ;;
  esac
  shift
done

# ---------------------------------------------------------------- 版本号

SEMVER_RE='^([0-9]+)\.([0-9]+)\.([0-9]+)(-[0-9A-Za-z.-]+)?$'

is_semver() { [[ "$1" =~ $SEMVER_RE ]]; }

# bump_version <version> <patch|minor|major>
# 预发布版本（1.2.0-beta.1）的 patch 会得到正式版 1.2.0
bump_version() {
  local version="$1" kind="$2"
  [[ "$version" =~ $SEMVER_RE ]] || die "invalid version: $version"
  local major="${BASH_REMATCH[1]}" minor="${BASH_REMATCH[2]}" patch="${BASH_REMATCH[3]}" pre="${BASH_REMATCH[4]}"
  case "$kind" in
    major) echo "$((major + 1)).0.0" ;;
    minor) echo "$major.$((minor + 1)).0" ;;
    patch)
      if [ -n "$pre" ]; then
        echo "$major.$minor.$patch"
      else
        echo "$major.$minor.$((patch + 1))"
      fi
      ;;
    *) die "unknown bump kind: $kind" ;;
  esac
}

resolve_version() {
  local current="$1" spec="$2"
  case "$spec" in
    "") echo "$current" ;;
    patch | minor | major) bump_version "$current" "$spec" ;;
    v*) resolve_version "$current" "${spec#v}" ;;
    *)
      is_semver "$spec" || die "invalid version: $spec (expected x.y.z or patch|minor|major)"
      echo "$spec"
      ;;
  esac
}

# ---------------------------------------------------------------- 依赖与预检

require() { command -v "$1" >/dev/null 2>&1 || die "$1 is required but not installed"; }
require git
require cargo
[ "$SKIP_CHECKS" -eq 1 ] || require pnpm
[ "$WAIT" -eq 0 ] || require gh

CURRENT="$(cargo xtask version)"
VERSION="$(resolve_version "$CURRENT" "$TARGET")"
TAG="v$VERSION"

if [ "$NEXT" = "none" ]; then
  NEXT_VERSION=""
else
  NEXT_VERSION="$(resolve_version "$VERSION" "$NEXT")"
  [ "$NEXT_VERSION" != "$VERSION" ] || die "next version must differ from $VERSION (use --next none to keep it)"
fi

# 发布说明：上一个 v* 标签到当前 HEAD 的提交历史
NOTES_FILE="$(mktemp -t toolforge-notes.XXXXXX)"
trap 'rm -f "$NOTES_FILE"' EXIT
cargo xtask notes HEAD "$TAG" >"$NOTES_FILE"

if [ "$NOTES_ONLY" -eq 1 ]; then
  cat "$NOTES_FILE"
  exit 0
fi

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
[ "$BRANCH" != "HEAD" ] || die "detached HEAD; check out a branch first"
[ -z "$(git status --porcelain)" ] || die "working tree is not clean; commit or stash your changes first"
git remote get-url "$REMOTE" >/dev/null 2>&1 || die "git remote '$REMOTE' does not exist"

step "Checking $REMOTE"
git fetch --quiet --tags "$REMOTE" || die "failed to fetch from $REMOTE"
if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  die "tag $TAG already exists locally"
fi
if [ -n "$(git ls-remote --tags "$REMOTE" "refs/tags/$TAG")" ]; then
  die "tag $TAG already exists on $REMOTE"
fi
UPSTREAM="$(git rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null || true)"
if [ -n "$UPSTREAM" ]; then
  BEHIND="$(git rev-list --count "HEAD..$UPSTREAM")"
  [ "$BEHIND" -eq 0 ] || die "$BRANCH is $BEHIND commit(s) behind $UPSTREAM; pull first"
else
  warn "$BRANCH has no upstream branch; skipping the up-to-date check"
fi
ok "$TAG is available and $BRANCH is up to date"

# ---------------------------------------------------------------- 计划

step "Release plan"
info "branch        ${C_BOLD}$BRANCH${C_RESET}"
info "remote        $REMOTE"
if [ "$VERSION" = "$CURRENT" ]; then
  info "version       ${C_BOLD}$VERSION${C_RESET} (current)"
else
  info "version       $CURRENT → ${C_BOLD}$VERSION${C_RESET} (commit: chore(release): $TAG)"
fi
info "tag           ${C_BOLD}$TAG${C_RESET} → push to $REMOTE (triggers $WORKFLOW)"
info "checks        $([ "$SKIP_CHECKS" -eq 1 ] && echo skipped || echo 'cargo xtask check')"
info "wait for CI   $([ "$WAIT" -eq 1 ] && echo yes || echo no)"
if [ -n "$NEXT_VERSION" ]; then
  info "next version  ${C_BOLD}$NEXT_VERSION${C_RESET} (commit: chore(release): start $NEXT_VERSION development)"
else
  info "next version  unchanged"
fi
info "push branch   $([ "$PUSH_BRANCH" -eq 1 ] && echo yes || echo no)"

step "Release notes"
sed "s/^/    ${C_DIM}│${C_RESET} /" "$NOTES_FILE"

if [ "$DRY_RUN" -eq 1 ]; then
  printf '\n%sDry run%s — nothing was changed.\n' "$C_YELLOW" "$C_RESET"
  exit 0
fi

if [ "$ASSUME_YES" -eq 0 ]; then
  printf '\nProceed with releasing %s%s%s? [y/N] ' "$C_BOLD" "$TAG" "$C_RESET"
  read -r answer
  case "$answer" in
    y | Y | yes | YES) ;;
    *) die "aborted" ;;
  esac
fi

# ---------------------------------------------------------------- 执行

TAG_CREATED=0
TAG_PUSHED=0
on_error() {
  local code=$?
  if [ "$TAG_CREATED" -eq 1 ] && [ "$TAG_PUSHED" -eq 0 ]; then
    git tag -d "$TAG" >/dev/null 2>&1 && warn "removed local tag $TAG because the release did not complete"
  fi
  rm -f "$NOTES_FILE"
  exit "$code"
}
trap on_error ERR

if [ "$SKIP_CHECKS" -eq 0 ]; then
  step "Running checks"
  cargo xtask check
fi

if [ "$VERSION" != "$CURRENT" ]; then
  step "Updating version to $VERSION"
  cargo xtask bump "$VERSION"
  git add package.json apps/desktop/package.json Cargo.toml Cargo.lock
  git commit --quiet -m "chore(release): $TAG"
  ok "committed chore(release): $TAG"
fi

step "Creating tag $TAG"
git tag -a "$TAG" --cleanup=verbatim -F "$NOTES_FILE"
TAG_CREATED=1
ok "created annotated tag $TAG"

step "Pushing $TAG to $REMOTE"
git push --quiet "$REMOTE" "refs/tags/$TAG"
TAG_PUSHED=1
ok "pushed $TAG; the release workflow has been triggered"

if [ "$WAIT" -eq 1 ]; then
  step "Waiting for the release workflow"
  RUN_ID=""
  for _ in $(seq 1 30); do
    RUN_ID="$(gh run list --workflow "$WORKFLOW" --branch "$TAG" --limit 1 --json databaseId --jq '.[0].databaseId' 2>/dev/null || true)"
    [ -n "$RUN_ID" ] && break
    sleep 5
  done
  [ -n "$RUN_ID" ] || die "could not find the $WORKFLOW run for $TAG; check GitHub Actions"
  info "run $RUN_ID"
  if ! gh run watch "$RUN_ID" --exit-status --interval 15; then
    die "release workflow failed; the version was not bumped. Inspect it with: gh run view $RUN_ID --log-failed"
  fi
  ok "release workflow succeeded"
fi

if [ -n "$NEXT_VERSION" ]; then
  step "Starting $NEXT_VERSION development"
  cargo xtask bump "$NEXT_VERSION"
  git add package.json apps/desktop/package.json Cargo.toml Cargo.lock
  git commit --quiet -m "chore(release): start $NEXT_VERSION development"
  ok "committed chore(release): start $NEXT_VERSION development"
fi

if [ "$PUSH_BRANCH" -eq 1 ]; then
  step "Pushing $BRANCH to $REMOTE"
  git push --quiet "$REMOTE" "$BRANCH"
  ok "pushed $BRANCH"
fi

trap - ERR
printf '\n%sReleased %s%s\n' "$C_GREEN" "$TAG" "$C_RESET"
info "Release: https://github.com/devlive-community/toolforge/releases/tag/$TAG"
if [ "$PUSH_BRANCH" -eq 0 ]; then
  info "Push the release commits when ready: git push $REMOTE $BRANCH"
fi
