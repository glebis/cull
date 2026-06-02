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

git fetch "$remote" "$target_branch" "$feature_branch" --prune
ensure_branch "$target_branch"
ensure_branch "$feature_branch"

git checkout "$target_branch"
git pull --ff-only "$remote" "$target_branch"
git merge --no-ff "$feature_branch" -m "Merge branch '$feature_branch'"

npm run check
npm test
npm run build

if ! bd sync; then
  bd vc status
fi

git push origin "$target_branch"

head_sha="$(git rev-parse HEAD)"
if command -v gh >/dev/null 2>&1; then
  printf 'Waiting for main CI for %s...\n' "$head_sha"
  run_id=""
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    run_id="$(gh run list \
      --workflow CI \
      --branch "$target_branch" \
      --commit "$head_sha" \
      --limit 1 \
      --json databaseId \
      --jq '.[0].databaseId // ""')"
    if [[ -n "$run_id" ]]; then
      break
    fi
    sleep 6
  done

  if [[ -n "$run_id" ]]; then
    gh run watch "$run_id" --exit-status
  else
    printf 'No CI run found yet for %s. Check GitHub Actions manually.\n' "$head_sha" >&2
  fi
fi
