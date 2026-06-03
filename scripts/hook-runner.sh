#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

hook_name="${1:-}"
if [ -z "$hook_name" ]; then
  echo "Usage: $0 <hook-name> [hook args...]" >&2
  exit 2
fi
shift || true

if [ "${CULL_HOOK_SKIP_CHECKS:-0}" = "1" ]; then
  echo "Cull hook: skipping checks because CULL_HOOK_SKIP_CHECKS=1"
  exit 0
fi

case "$hook_name" in
  pre-commit)
    tier="${CULL_PRE_COMMIT_TIER:-hook}"
    ;;
  pre-push)
    tier="${CULL_PRE_PUSH_TIER:-full}"
    ;;
  *)
    exit 0
    ;;
esac

echo "Cull hook: running ${tier} preflight for ${hook_name}"
bash scripts/preflight.sh "$tier"
