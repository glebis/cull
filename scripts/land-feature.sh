#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: npm run land:feature -- <feature-branch> [target-branch]\n' >&2
  printf 'Example: npm run land:feature -- codex/site-publishing-control-center\n' >&2
}

feature_branch="${1:-}"
target_branch="${2:-main}"
remote="${REMOTE:-origin}"

if [[ -z "$feature_branch" || "$feature_branch" == "$target_branch" ]]; then
  usage
  exit 2
fi

for command_name in git gh npm; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    printf 'Required command not found: %s\n' "$command_name" >&2
    exit 1
  fi
done

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

if [[ -n "$(git status --porcelain)" ]]; then
  printf 'Working tree must be clean before landing a feature branch.\n' >&2
  git status --short
  exit 1
fi

ensure_branch() {
  local branch="$1"
  if git show-ref --verify --quiet "refs/heads/$branch"; then
    return
  fi
  if git show-ref --verify --quiet "refs/remotes/$remote/$branch"; then
    git branch --track "$branch" "$remote/$branch"
    return
  fi
  printf 'Branch not found locally or on %s: %s\n' "$remote" "$branch" >&2
  exit 1
}

git fetch "$remote" --prune
ensure_branch "$target_branch"
ensure_branch "$feature_branch"

if ! git merge-base --is-ancestor "$target_branch" "$remote/$target_branch"; then
  printf 'Local %s contains commits not present on %s/%s.\n' \
    "$target_branch" "$remote" "$target_branch" >&2
  printf 'Preserve or reconcile that work before landing this pull request.\n' >&2
  exit 1
fi

unique_commits="$(git rev-list --count "$remote/$target_branch..$feature_branch")"
if [[ "$unique_commits" -eq 0 ]]; then
  printf 'Feature branch has no commits ahead of %s/%s: %s\n' \
    "$remote" "$target_branch" "$feature_branch" >&2
  exit 1
fi

printf 'Commits proposed for %s:\n' "$target_branch"
git log "$remote/$target_branch..$feature_branch" --oneline
printf 'Changed-file summary:\n'
git diff --stat "$remote/$target_branch...$feature_branch"

npm run preflight -- full

feature_sha="$(git rev-parse "$feature_branch")"
git push --set-upstream "$remote" "$feature_branch"

repo_slug="$(gh repo view --json nameWithOwner --jq '.nameWithOwner')"
pr_number="$(gh pr list \
  --repo "$repo_slug" \
  --head "$feature_branch" \
  --base "$target_branch" \
  --state open \
  --json number \
  --jq '.[0].number // ""')"

if [[ -z "$pr_number" ]]; then
  pr_url="$(gh pr create \
    --repo "$repo_slug" \
    --base "$target_branch" \
    --head "$feature_branch" \
    --fill)"
  pr_number="$(gh pr view "$pr_url" \
    --repo "$repo_slug" \
    --json number \
    --jq '.number')"
  printf 'Created pull request #%s: %s\n' "$pr_number" "$pr_url"
else
  printf 'Updated pull request #%s from %s.\n' "$pr_number" "$feature_branch"
fi

read -r pr_head_sha pr_base_branch < <(
  gh pr view "$pr_number" \
    --repo "$repo_slug" \
    --json headRefOid,baseRefName \
    --jq '[.headRefOid, .baseRefName] | @tsv'
)

if [[ "$pr_head_sha" != "$feature_sha" || "$pr_base_branch" != "$target_branch" ]]; then
  printf 'Pull request #%s does not match the requested head and base.\n' "$pr_number" >&2
  printf 'Expected %s -> %s; found %s -> %s.\n' \
    "$feature_sha" "$target_branch" "$pr_head_sha" "$pr_base_branch" >&2
  exit 1
fi

required_check_count=""
check_discovery_attempts="${CULL_REQUIRED_CHECK_DISCOVERY_ATTEMPTS:-20}"
check_discovery_interval="${CULL_REQUIRED_CHECK_DISCOVERY_INTERVAL:-6}"
for ((attempt = 1; attempt <= check_discovery_attempts; attempt += 1)); do
  required_check_count="$(
    gh pr checks "$pr_number" \
      --repo "$repo_slug" \
      --required \
      --json name \
      --jq 'length' 2>/dev/null || true
  )"
  if [[ "$required_check_count" =~ ^[1-9][0-9]*$ ]]; then
    break
  fi
  if ((attempt < check_discovery_attempts)); then
    sleep "$check_discovery_interval"
  fi
done

if [[ ! "$required_check_count" =~ ^[1-9][0-9]*$ ]]; then
  printf 'No required checks are configured or visible for pull request #%s.\n' \
    "$pr_number" >&2
  printf 'Refusing to merge without an explicit required-check gate.\n' >&2
  exit 1
fi

printf 'Waiting for %s required checks on pull request #%s...\n' \
  "$required_check_count" "$pr_number"
gh pr checks "$pr_number" \
  --repo "$repo_slug" \
  --required \
  --watch \
  --fail-fast \
  --interval 15

gh pr merge "$pr_number" \
  --repo "$repo_slug" \
  --merge \
  --match-head-commit "$feature_sha"

pr_state=""
merge_sha=""
merge_wait_attempts="${CULL_MERGE_WAIT_ATTEMPTS:-180}"
merge_wait_interval="${CULL_MERGE_WAIT_INTERVAL:-10}"
for ((attempt = 1; attempt <= merge_wait_attempts; attempt += 1)); do
  read -r pr_state merge_sha < <(
    gh pr view "$pr_number" \
      --repo "$repo_slug" \
      --json state,mergeCommit \
      --jq '[.state, .mergeCommit.oid // ""] | @tsv'
  )
  if [[ "$pr_state" == "MERGED" && -n "$merge_sha" ]]; then
    break
  fi
  if [[ "$pr_state" == "CLOSED" ]]; then
    break
  fi
  if ((attempt < merge_wait_attempts)); then
    printf 'Pull request #%s is queued; waiting for GitHub to merge it...\n' "$pr_number"
    sleep "$merge_wait_interval"
  fi
done

if [[ "$pr_state" != "MERGED" || -z "$merge_sha" ]]; then
  printf 'Pull request #%s was not confirmed merged; local %s was not moved.\n' \
    "$pr_number" "$target_branch" >&2
  exit 1
fi

git fetch "$remote" "$target_branch"
git switch "$target_branch"
git merge --ff-only "$remote/$target_branch"

if git ls-remote --exit-code --heads "$remote" "$feature_branch" >/dev/null 2>&1; then
  gh api \
    --method DELETE \
    "repos/$repo_slug/git/refs/heads/$feature_branch"
fi

printf 'Landed pull request #%s at %s and fast-forwarded local %s.\n' \
  "$pr_number" "$merge_sha" "$target_branch"
