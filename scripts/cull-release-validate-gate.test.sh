#!/usr/bin/env bash
# Self-test for cull-release-validate-gate.sh.
#
# Verifies the shim's decision logic in isolation by reading its stdout/stderr
# from a controlled environment. We do NOT invoke a real `preflight.sh` here;
# the fall-through path is exercised manually.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
shim="$repo_root/scripts/cull-release-validate-gate.sh"
[[ -x "$shim" ]] || { echo "shim not executable" >&2; exit 1; }

tmp="$(mktemp -d -t cv-gate-test)"
trap 'rm -rf "$tmp"' EXIT

current_sha="$(git -C "$repo_root" rev-parse HEAD)"
short_sha="${current_sha:0:7}"

write_artifact() {
  local p=$1 sha=$2 status=$3 run=$4
  mkdir -p "$(dirname "$p")"
  printf '{"schema":"cull.release.validate.v1","sha":"%s","status":"%s","runId":%s,"timestamp":"2026-08-20T18:00:00Z","ref":"refs/heads/main","event":"push"}\n' "$sha" "$status" "$run" >"$p"
}

pass_count=0
fail() { printf 'not ok - %s\n' "$1" >&2; exit 1; }

# === Decision-logic tests ===
# These run the shim with the fall-through disabled (a marker env var we set
# only for the test) so we can test the cache-matching logic without spending
# minutes running preflight.

# Helper: run shim with fall-through short-circuited (test mode),
# capture output.
run_capture() {
  local cache=$1
  local outfile="$tmp/_out"
  (
    cd "$repo_root"
    CULL_RELEASE_VALIDATE_CACHE="$cache" \
    CULL_RELEASE_VALIDATE_AUTO_FETCH=0 \
    CULL_RELEASE_VALIDATE_DISABLE_FOR_TEST=1 \
    bash "$shim" >"$outfile" 2>&1
  ) || true
  cat "$outfile"
}

# Case 1: matching passed artifact -> short-circuit
cache1="$tmp/case1"
write_artifact "$cache1/match.json" "$current_sha" "passed" 12345
out="$(run_capture "$cache1")"
[[ "$out" == *"CI blessed ${short_sha}"* ]] || fail "case1 expected blessed line; got: $out"
[[ "$out" == *"skipping local preflight"* ]] || fail "case1 missing skip message: $out"
pass_count=$((pass_count + 1))
echo "ok $pass_count - matching-passed-artifact-shortcircuits"

# Case 2: no artifact at all -> falls through (we just confirm the message)
cache2="$tmp/case2-empty"
mkdir -p "$cache2"
out="$(run_capture "$cache2")"
[[ "$out" == *"no matching CI artifact"* ]] || fail "case2 expected fall-through; got: $out"
pass_count=$((pass_count + 1))
echo "ok $pass_count - no-artifact-falls-through"

# Case 3: mismatched SHA -> falls through (must not match the wrong artifact)
cache3="$tmp/case3"
write_artifact "$cache3/wrong-sha.json" "0000000000000000000000000000000000000000" "passed" 1
out="$(run_capture "$cache3")"
[[ "$out" != *"skipping local preflight"* ]] || fail "case3 should not short-circuit on wrong-sha artifact"
[[ "$out" == *"no matching CI artifact"* ]] || fail "case3 expected fall-through; got: $out"
pass_count=$((pass_count + 1))
echo "ok $pass_count - mismatched-sha-falls-through"

# Case 4: failed (status != passed) artifact -> falls through
cache4="$tmp/case4"
write_artifact "$cache4/failed.json" "$current_sha" "failed" 1
out="$(run_capture "$cache4")"
[[ "$out" != *"skipping local preflight"* ]] || fail "case4 should not short-circuit on failed artifact"
pass_count=$((pass_count + 1))
echo "ok $pass_count - failed-artifact-falls-through"

# Case 5: cache directory is auto-created if missing
cache5="$tmp/case5-never-existed"
out="$(run_capture "$cache5")"
[[ -d "$cache5" ]] || fail "case5 did not auto-create cache directory"
pass_count=$((pass_count + 1))
echo "ok $pass_count - cache-dir-auto-created"

# Case 6: CULL_RELEASE_VALIDATE_DISABLE=1 always falls through (manual test path)
out="$(cd "$repo_root" && CULL_RELEASE_VALIDATE_DISABLE=1 CULL_RELEASE_VALIDATE_CACHE="$cache1" CULL_RELEASE_VALIDATE_AUTO_FETCH=0 CULL_RELEASE_VALIDATE_DISABLE_FOR_TEST=1 bash "$shim" 2>&1)"
[[ "$out" == *"disabled via CULL_RELEASE_VALIDATE_DISABLE"* ]] || fail "case6 expected disable message; got: $out"
pass_count=$((pass_count + 1))
echo "ok $pass_count - disable-flag-forces-fallthrough"

echo "1..$pass_count"
