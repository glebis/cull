#!/usr/bin/env bash
set -euo pipefail

if [ "$(uname -s)" != "Darwin" ]; then
  echo "Packaged interaction smoke requires the macOS WKWebView runtime" >&2
  exit 2
fi

repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
shots_dir="${CULL_NATIVE_SMOKE_SHOTS:-${RUNNER_TEMP:-/tmp}/cull-native-interaction-smoke}"
# Keep the runtime socket below macOS SUN_LEN, including the app identifier.
work_dir="${CULL_NATIVE_SMOKE_WORK:-$(mktemp -d /tmp/cs.XXXXXX)}"
smoke_home="$work_dir/h"
fixture_root="$work_dir/fixtures"
# Tauri resolves its macOS data directory from the HOME used at launch.
# Seed that exact directory so both processes see the same isolated database.
app_data="$smoke_home/Library/Application Support/com.glebkalinin.cull.interaction-smoke"
app_bundle="${CULL_NATIVE_SMOKE_APP:-$repo_root/src-tauri/target/release/bundle/macos/Cull.app}"
app_binary="$app_bundle/Contents/MacOS/cull"
app_log="$shots_dir/cull-native-interaction-smoke.log"
timeout_seconds="${CULL_NATIVE_SMOKE_TIMEOUT_SECONDS:-180}"

if [ -d "$app_data" ]; then
  if command -v trash >/dev/null 2>&1; then
    trash "$app_data"
  else
    echo "Refusing to reuse non-empty smoke app data without the trash command: $app_data" >&2
    exit 2
  fi
fi
mkdir -p "$shots_dir" "$smoke_home" "$fixture_root/Smoke Alpha" "$fixture_root/Smoke Beta" "$app_data"

cleanup() {
  if [ "${CULL_NATIVE_SMOKE_KEEP_DATA:-0}" != "1" ] && command -v trash >/dev/null 2>&1; then
    [ ! -d "$app_data" ] || trash "$app_data" >/dev/null 2>&1 || true
    [ ! -d "$work_dir" ] || trash "$work_dir" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

# Two distinct, valid PNGs make folder filtering and image navigation observable.
cp "$repo_root/design/icons/tahoe/masters-unmasked/cull-red.png" "$fixture_root/Smoke Alpha/alpha.png"
cp "$repo_root/design/icons/tahoe/masters-unmasked/cull-blue.png" "$fixture_root/Smoke Beta/beta.png"

if [ "${CULL_NATIVE_SMOKE_SKIP_BUILD:-0}" != "1" ]; then
  echo "[native-smoke] Building packaged production Cull.app"
  (
    cd "$repo_root"
    CULL_NATIVE_INTERACTION_SMOKE=1 npm run tauri -- build \
      --bundles app \
      --no-sign \
      --ci \
      --config tests/native/tauri.smoke.conf.json
  )
fi

if [ ! -x "$app_binary" ]; then
  echo "Packaged app executable is missing: $app_binary" >&2
  exit 2
fi

echo "[native-smoke] Seeding isolated database at $app_data"
HOME="$smoke_home" "$app_binary" \
  --db "$app_data/cull.db" \
  --app-data-dir "$app_data" \
  import_folder --folder-path "$fixture_root" >/dev/null

# The second process uses the same isolated database and must report a fresh
# restart assertion. A stale first-run success manifest cannot satisfy it.
for smoke_phase in initial restart; do
  echo "[native-smoke] Launching packaged WKWebView ($smoke_phase)"
  app_log="$shots_dir/cull-native-interaction-smoke-$smoke_phase.log"
  CULL_NATIVE_SMOKE_ACTIVE=1 HOME="$smoke_home" "$app_binary" >"$app_log" 2>&1 &
  app_pid="$!"
  deadline=$((SECONDS + timeout_seconds))
  timed_out=0
  while kill -0 "$app_pid" 2>/dev/null; do
    if [ "$SECONDS" -ge "$deadline" ]; then
      timed_out=1
      /usr/sbin/screencapture -x "$shots_dir/native-interaction-smoke-$smoke_phase-timeout.png" >/dev/null 2>&1 || true
      kill -TERM "$app_pid" >/dev/null 2>&1 || true
      break
    fi
    sleep 1
  done
  set +e
  wait "$app_pid"
  status="$?"
  set -e
  if [ "$timed_out" = "1" ]; then status=124; fi

  snapshot_root="$app_data/Agent Snapshots"
  result_manifest="$snapshot_root/native_interaction_smoke_result/manifest.json"
  if [ -d "$snapshot_root" ]; then
    while IFS= read -r artifact; do
      cp "$artifact" "$shots_dir/$smoke_phase-$(basename "$(dirname "$artifact")")-$(basename "$artifact")"
    done < <(find "$snapshot_root" -type f \( -name '*.png' -o -name 'manifest.json' \) -print)
  fi

  # Require both a clean process exit and this phase's persisted success.
  # A stale manifest or a crash after writing it cannot turn a failure into a pass.
  if [ "$timed_out" != "1" ] && [ -f "$result_manifest" ]; then
    if python3 - "$result_manifest" "$smoke_phase" <<'PYVERIFY'
import json, sys
with open(sys.argv[1]) as stream:
    manifest = json.load(stream)
result = manifest.get('smoke_result', {})
expected = 'selection-shortlist-save' if sys.argv[2] == 'initial' else 'selection-restart-persistence'
completed = result.get('completed')
if not (result.get('ok') is True and isinstance(completed, list)
        and all(isinstance(item, str) for item in completed) and expected in completed):
    raise SystemExit(1)
PYVERIFY
    then :; else status=1; fi
  else
    status=1
  fi
  if [ "$status" -ne 0 ]; then
    echo "[native-smoke] FAIL $smoke_phase (exit $status); artifacts: $shots_dir" >&2
    exit "$status"
  fi
done

echo "[native-smoke] PASS including process restart"
