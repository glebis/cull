#!/usr/bin/env bash
# Fetch the release-cull-validate artifact for the current HEAD from GitHub.
# Usage:
#   bash scripts/cull-release-validate-fetch.sh                # fetch latest main
#   bash scripts/cull-release-validate-fetch.sh --sha <sha>   # fetch for a specific SHA
#
# Writes to $CULL_RELEASE_VALIDATE_CACHE (default ~/.cache/cull-release-validate/).
# Exits 0 if an artifact is downloaded and its status is "passed" for the requested SHA.
# Exits non-zero (silently recoverable) if no artifact is available — caller should
# fall back to local preflight.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

cache="${CULL_RELEASE_VALIDATE_CACHE:-$HOME/.cache/cull-release-validate}"
mkdir -p "$cache"

requested_sha=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --sha) requested_sha="$2"; shift 2 ;;
    *) printf 'unknown arg: %s\n' "$1" >&2; exit 2 ;;
  esac
done

if [[ -z "$requested_sha" ]]; then
  requested_sha="$(git rev-parse HEAD)"
fi

if ! command -v gh >/dev/null 2>&1; then
  printf 'gh CLI not available; skipping artifact fetch\n' >&2
  exit 1
fi

if ! gh auth status >/dev/null 2>&1; then
  printf 'gh not authenticated; skipping artifact fetch\n' >&2
  exit 1
fi

repo="$(gh repo view --json nameWithOwner -q .nameWithOwner)"
printf 'validate-fetch: repo=%s sha=%s\n' "$repo" "${requested_sha:0:7}" >&2

run_id="$(gh run list \
  --repo "$repo" \
  --workflow release-cull-validate.yml \
  --branch main \
  --status success \
  --limit 20 \
  --json databaseId,headSha \
  -q '.[] | select(.headSha == "'"$requested_sha"'") | .databaseId' | head -n1)"

if [[ -z "$run_id" ]]; then
  printf 'validate-fetch: no successful run for %s on main; will fall back to local\n' "${requested_sha:0:7}" >&2
  exit 1
fi

printf 'validate-fetch: run_id=%s\n' "$run_id" >&2

artifact_name="$(gh api "repos/$repo/actions/runs/$run_id/artifacts" \
  --jq '.artifacts[] | select(.name | startswith("cull-release-validate-")) | .name' | head -n1)"

if [[ -z "$artifact_name" ]]; then
  printf 'validate-fetch: no matching artifact for run %s\n' "$run_id" >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT
gh run download "$run_id" --repo "$repo" --name "$artifact_name" --dir "$tmp_dir" >/dev/null

downloaded="$tmp_dir/$artifact_name"
if [[ ! -s "$downloaded" ]]; then
  printf 'validate-fetch: downloaded artifact is empty\n' >&2
  exit 1
fi

mv "$downloaded" "$cache/$artifact_name"
printf 'validate-fetch: cached %s\n' "$artifact_name" >&2
