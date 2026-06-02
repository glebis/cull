#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

run_checks=1
release_tier=0
dry_run=0
push_remote=1

usage() {
  cat <<'EOF'
Usage: scripts/land-session.sh [--skip-checks] [--release] [--dry-run] [--no-push]

Verifies and performs Cull's session landing sequence:
  1. Require a clean git worktree.
  2. Run the full or release preflight tier unless skipped.
  3. Report bd version-control state and push bd Dolt data when configured.
  4. Pull with rebase, push the current branch, and print final status.

The script does not delete, reset, or clean user files.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --skip-checks)
      run_checks=0
      ;;
    --release)
      release_tier=1
      ;;
    --dry-run)
      dry_run=1
      ;;
    --no-push)
      push_remote=0
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

run() {
  printf '+'
  printf ' %q' "$@"
  printf '\n'
  if [ "$dry_run" = "1" ]; then
    return 0
  fi
  "$@"
}

run_shell() {
  echo "+ $*"
  if [ "$dry_run" = "1" ]; then
    return 0
  fi
  "$@"
}

require_clean_worktree() {
  local status
  status="$(git status --porcelain)"
  if [ -n "$status" ]; then
    if [ "$dry_run" = "1" ]; then
      echo "Cull landing dry run: real mode would fail because git worktree is not clean." >&2
      git status --short >&2
      return 0
    fi
    echo "Cull landing failed: git worktree is not clean." >&2
    git status --short >&2
    echo "Commit, stash, or intentionally leave this session unlanded before running the landing script." >&2
    exit 1
  fi
}

bd_command_exists() {
  bash scripts/bd.sh "$1" --help >/dev/null 2>&1
}

sync_bd_state() {
  if ! bash scripts/bd.sh --help >/dev/null 2>&1; then
    echo "bd not found; skipping bd status" >&2
    return 0
  fi

  run bash scripts/bd.sh vc status

  if bd_command_exists sync; then
    run bash scripts/bd.sh sync
    return 0
  fi

  echo "bd sync is unavailable; using bd vc status and bd dolt remote inspection."
  run bash scripts/bd.sh vc status

  local remotes
  remotes="$(bash scripts/bd.sh dolt remote list 2>/dev/null || true)"
  if [ -n "$remotes" ] && ! printf '%s\n' "$remotes" | grep -q "No remotes configured"; then
    run bash scripts/bd.sh dolt push
  else
    echo "No bd Dolt remote configured; bd changes must be carried by tracked .beads JSONL files."
  fi
}

echo "Cull landing: initial status"
run git status --short --branch
require_clean_worktree

if [ "$run_checks" = "1" ]; then
  if [ "$release_tier" = "1" ]; then
    run bash scripts/preflight.sh release
  else
    run bash scripts/preflight.sh full
  fi
else
  echo "Cull landing: skipping quality gates because --skip-checks was set"
fi

sync_bd_state

run git pull --rebase
if [ "$push_remote" = "1" ]; then
  run git push
else
  echo "Cull landing: skipping git push because --no-push was set"
fi

echo "Cull landing: final status"
run git status --short --branch
sync_bd_state
