#!/usr/bin/env bash
# Cull release gate shim.
#
# If a recent release-cull-validate workflow run has blessed the current
# HEAD, exit 0 without re-running the heavy preflight suite locally.
# Otherwise fall back to the configured local gate command.
#
# Configuration:
#   release.config.json -> gate: "bash scripts/cull-release-validate-gate.sh"
#   CULL_RELEASE_VALIDATE_DISABLE=1     -> force local fallback (debug)
#   CULL_RELEASE_VALIDATE_AUTO_FETCH=0  -> disable auto-fetch (manual only)
#   CULL_RELEASE_VALIDATE_CACHE=<path>  -> override cache dir
#
# Output: prints the validate manifest status line on stdout when CI blessed it,
#         prints a one-liner on stderr explaining which path was taken.
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"
export CI="${CI:-true}"

artifact_root="${CULL_RELEASE_VALIDATE_CACHE:-$HOME/.cache/cull-release-validate}"
mkdir -p "$artifact_root"

source_sha="$(git rev-parse HEAD)"

disable="${CULL_RELEASE_VALIDATE_DISABLE:-0}"
if [[ "$disable" == "1" ]]; then
  printf 'validate-gate: disabled via CULL_RELEASE_VALIDATE_DISABLE; running local preflight\n' >&2
  if [[ "${CULL_RELEASE_VALIDATE_DISABLE_FOR_TEST:-0}" == "1" ]]; then
    exit 0
  fi
  exec bash scripts/preflight.sh release
fi

matches_sha() {
  # Usage: matches_sha <manifest-path> <sha> -> 0 if matches and passed, 1 otherwise
  local path="$1"
  local sha="$2"
  python3 - "$path" "$sha" <<'PY'
import json, sys
path, sha = sys.argv[1], sys.argv[2]
try:
    with open(path) as fh:
        data = json.load(fh)
except Exception:
    sys.exit(1)
sys.exit(0 if (data.get('status') == 'passed' and data.get('sha') == sha) else 1)
PY
}

find_blessed_manifest() {
  local sha="$1"
  local candidate
  for candidate in "$artifact_root"/*.json; do
    [[ -e "$candidate" ]] || continue
    if matches_sha "$candidate" "$sha"; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

manifest=""
if find_blessed_manifest "$source_sha" >/dev/null 2>&1; then
  manifest="$(find_blessed_manifest "$source_sha")"
fi

if [[ -z "$manifest" ]] && [[ "${CULL_RELEASE_VALIDATE_AUTO_FETCH:-1}" == "1" ]] && command -v gh >/dev/null 2>&1; then
  if bash scripts/cull-release-validate-fetch.sh --sha "$source_sha" 2>/dev/null; then
    if find_blessed_manifest "$source_sha" >/dev/null 2>&1; then
      manifest="$(find_blessed_manifest "$source_sha")"
    fi
  fi
fi

if [[ -n "$manifest" ]]; then
  status_line="$(python3 - "$manifest" <<'PY'
import json, sys
with open(sys.argv[1]) as fh:
    data = json.load(fh)
sha = data.get('sha', '')
short = sha[:7] if sha else 'unknown'
ts = data.get('timestamp', '')
run = data.get('runId', '')
print(f"validate-gate: CI blessed {short} (run {run}, {ts}) — skipping local preflight")
PY
)"
  printf '%s\n' "$status_line"
  exit 0
fi

printf 'validate-gate: no matching CI artifact for HEAD=%s; running local preflight\n' "${source_sha:0:7}" >&2
if [[ "${CULL_RELEASE_VALIDATE_DISABLE_FOR_TEST:-0}" == "1" ]]; then
  printf 'validate-gate: test-mode short-circuit (CULL_RELEASE_VALIDATE_DISABLE_FOR_TEST=1)\n' >&2
  exit 0
fi
exec bash scripts/preflight.sh release
