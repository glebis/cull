#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
verifier="$repo_root/scripts/verify-release-artifacts.sh"
tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/cull-artifact-test.XXXXXX")"
# shellcheck source=scripts/safe-cleanup-private.sh
source "$repo_root/scripts/safe-cleanup-private.sh"
trap 'safe_cleanup_private "$tmp_root" "$(dirname "$tmp_root")" "cull-artifact-test." >/dev/null 2>&1 || true' EXIT
real_node="$(command -v node)"

pass_count=0

fail() {
  printf 'not ok - %s\n' "$1" >&2
  exit 1
}

make_tools() {
  local bin_dir=$1
  mkdir -p "$bin_dir"

  cat >"$bin_dir/codesign" <<'EOF'
#!/usr/bin/env bash
exit "${FAKE_CODESIGN_STATUS:-0}"
EOF
  cat >"$bin_dir/spctl" <<'EOF'
#!/usr/bin/env bash
exit "${FAKE_SPCTL_STATUS:-0}"
EOF
  cat >"$bin_dir/xcrun" <<'EOF'
#!/usr/bin/env bash
if [[ "$1 $2" == "stapler validate" ]]; then
  exit "${FAKE_STAPLER_STATUS:-0}"
fi
exit 1
EOF
  cat >"$bin_dir/hdiutil" <<'EOF'
#!/usr/bin/env bash
if [[ "$1" == "attach" ]]; then
  mountpoint=""
  while [[ $# -gt 0 ]]; do
    if [[ "$1" == "-mountpoint" ]]; then
      mountpoint=$2
      shift 2
    else
      shift
    fi
  done
  printf '%s\n' "$mountpoint" >"$FAKE_MOUNT_PATH_FILE"
  mkdir -p "$mountpoint/Cull.app/Contents/MacOS"
  : >"$mountpoint/Cull.app/Contents/Info.plist"
  : >"$mountpoint/Cull.app/Contents/MacOS/Cull"
  if [[ "${FAKE_ATTACH_STATUS:-0}" != 0 ]]; then
    exit "$FAKE_ATTACH_STATUS"
  fi
  exit 0
fi
if [[ "$1" == "detach" ]]; then
  [[ -z "${FAKE_DETACH_MARKER:-}" ]] || printf 'detached\n' >>"$FAKE_DETACH_MARKER"
  exit "${FAKE_DETACH_STATUS:-0}"
fi
exit 1
EOF
  cat >"$bin_dir/mount" <<'EOF'
#!/usr/bin/env bash
if [[ "${FAKE_MOUNT_ACTIVE:-0}" == 1 && -s "$FAKE_MOUNT_PATH_FILE" ]]; then
  printf '/dev/disk-test on %s (apfs, local, read-only)\n' "$(cat "$FAKE_MOUNT_PATH_FILE")"
fi
exit "${FAKE_MOUNT_STATUS:-0}"
EOF
  cat >"$bin_dir/plutil" <<'EOF'
#!/usr/bin/env bash
case "$2" in
  CFBundleShortVersionString) printf '%s\n' "${FAKE_BUNDLE_VERSION:-0.2.6}" ;;
  CFBundleExecutable) printf '%s\n' 'Cull' ;;
  *) exit 1 ;;
esac
EOF
  cat >"$bin_dir/lipo" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "${FAKE_ARCHS:-arm64}"
EOF
  cat >"$bin_dir/minisign" <<'EOF'
#!/usr/bin/env bash
args=" $* "
[[ "$args" == *" -Vm "* ]] || exit 41
[[ "$args" == *" -x "* ]] || exit 42
[[ "$args" == *" -p "* ]] || exit 43
x_path=""
while [[ $# -gt 0 ]]; do
  if [[ "$1" == '-x' ]]; then
    x_path=$2
    shift 2
  else
    shift
  fi
done
grep -q '^untrusted comment: signature from tauri secret key$' "$x_path" || exit 44
if [[ "${FAKE_MUTATE_SOURCE:-0}" == 1 ]]; then
  printf 'mutated-after-snapshot\n' >"$FAKE_SOURCE_DIR/Cull_aarch64.app.tar.gz"
fi
exit "${FAKE_MINISIGN_STATUS:-0}"
EOF
  ln -s "$real_node" "$bin_dir/node"
  for required_tool in bash shasum stat awk wc tr grep dirname mktemp mkdir cat git date cp find; do
    if [[ ! -e "$bin_dir/$required_tool" ]]; then
      ln -s "$(command -v "$required_tool")" "$bin_dir/$required_tool"
    fi
  done
  chmod +x "$bin_dir"/*
}

make_artifacts() {
  local dir=$1
  local version=${2:-0.2.6}
  mkdir -p "$dir"
  printf 'dmg\n' >"$dir/Cull_${version}_aarch64.dmg"
  printf 'archive\n' >"$dir/Cull_aarch64.app.tar.gz"
  node - "$dir/Cull_aarch64.app.tar.gz.sig" <<'NODE'
const fs = require('node:fs');
const text = 'untrusted comment: signature from tauri secret key\nRUTESTSIGNATUREVALUE==\ntrusted comment: timestamp:1\nRUTESTTRUSTEDVALUE==\n';
fs.writeFileSync(process.argv[2], Buffer.from(text, 'utf8').toString('base64') + '\n');
NODE
  node - "$dir/latest.json" "$version" <<'NODE'
const fs = require('node:fs');
const [path, version] = process.argv.slice(2);
fs.writeFileSync(path, JSON.stringify({
  version,
  notes: 'test',
  pub_date: '2026-07-10T00:00:00Z',
  platforms: {
    'darwin-aarch64': {
      signature: fs.readFileSync(path.replace(/latest\.json$/, 'Cull_aarch64.app.tar.gz.sig'), 'utf8').trim(),
      url: 'https://github.com/glebis/cull/releases/download/v0.2.6/Cull_aarch64.app.tar.gz'
    }
  }
}) + '\n');
NODE
}

run_case() {
  local name=$1
  local expected=$2
  local setup=${3:-:}
  local case_dir="$tmp_root/$name"
  local artifacts="$case_dir/artifacts"
  local output="$case_dir/output"
  local bin_dir="$case_dir/bin"
  local runner_temp="$case_dir/runner-temp"
  mkdir -p "$case_dir" "$runner_temp"
  make_tools "$bin_dir"
  make_artifacts "$artifacts"
  "$setup" "$artifacts" "$output"
  local case_path="$bin_dir"
  local verifier_args=(
    --artifact-dir "$artifacts"
    --version 0.2.6
    --tag v0.2.6
    --commit 0123456789abcdef0123456789abcdef01234567
  )
  if [[ -n "${CASE_TAG_OBJECT_SHA:-}" ]]; then
    verifier_args+=(--tag-object-sha "$CASE_TAG_OBJECT_SHA")
  fi
  verifier_args+=(--run-id 123 --out "$output")

  set +e
  PATH="$case_path" \
    RUNNER_TEMP="$runner_temp" \
    FAKE_CODESIGN_STATUS="${CASE_CODESIGN_STATUS:-0}" \
    FAKE_SPCTL_STATUS="${CASE_SPCTL_STATUS:-0}" \
    FAKE_MINISIGN_STATUS="${CASE_MINISIGN_STATUS:-0}" \
    FAKE_BUNDLE_VERSION="${CASE_BUNDLE_VERSION:-0.2.6}" \
    FAKE_ARCHS="${CASE_ARCHS:-arm64}" \
    FAKE_STAPLER_STATUS="${CASE_STAPLER_STATUS:-0}" \
    FAKE_DETACH_STATUS="${CASE_DETACH_STATUS:-0}" \
    FAKE_ATTACH_STATUS="${CASE_ATTACH_STATUS:-0}" \
    FAKE_DETACH_MARKER="$case_dir/detach-marker" \
    FAKE_MUTATE_SOURCE="${CASE_MUTATE_SOURCE:-0}" \
    FAKE_SOURCE_DIR="$artifacts" \
    FAKE_MOUNT_ACTIVE="${CASE_MOUNT_ACTIVE:-0}" \
    FAKE_MOUNT_STATUS="${CASE_MOUNT_STATUS:-0}" \
    FAKE_MOUNT_PATH_FILE="$case_dir/mount-path" \
    CULL_RELEASE_TEST_MODE=1 \
    CULL_VERIFY_TEST_RACE_PUBLISHED_NAME="${CASE_RACE_PUBLISHED_NAME:-}" \
    CULL_VERIFY_TEST_SIGNAL_DURING_PUBLISH="${CASE_SIGNAL_DURING_PUBLISH:-}" \
    CULL_VERIFY_TEST_RACE_DESTINATION_NAME="${CASE_RACE_DESTINATION_NAME:-}" \
    bash "$verifier" "${verifier_args[@]}" >"$case_dir/stdout" 2>"$case_dir/stderr"
  local status=$?
  printf '%s\n' "$status" >"$case_dir/status"
  set -e

  if [[ "$expected" == pass && $status -ne 0 ]]; then
    cat "$case_dir/stderr" >&2
    fail "$name expected success"
  fi
  if [[ "$expected" == fail && $status -eq 0 ]]; then
    fail "$name expected failure"
  fi
  if [[ "$expected" == fail && "${CASE_ALLOW_EXISTING_EVIDENCE:-0}" != 1 && ( -e "$output/release-provenance.json" || -e "$output/checksums.txt" ) ]]; then
    fail "$name left success evidence after failure"
  fi
  pass_count=$((pass_count + 1))
  printf 'ok %d - %s\n' "$pass_count" "$name"
}

setup_valid() { :; }
setup_missing_signature() { unlink "$1/Cull_aarch64.app.tar.gz.sig"; }
setup_extra_asset() { printf 'unexpected\n' >"$1/extra.zip"; }
setup_stale_metadata() {
  node - "$1/latest.json" <<'NODE'
const fs = require('node:fs');
const path = process.argv[2];
const data = JSON.parse(fs.readFileSync(path));
data.version = '0.2.5';
fs.writeFileSync(path, JSON.stringify(data));
NODE
}
setup_wrong_asset_url() {
  node - "$1/latest.json" <<'NODE'
const fs = require('node:fs');
const path = process.argv[2];
const data = JSON.parse(fs.readFileSync(path));
data.platforms['darwin-aarch64'].url = 'https://example.test/Other.tar.gz';
fs.writeFileSync(path, JSON.stringify(data));
NODE
}
setup_signature_mismatch() {
  printf 'ZGlmZmVyZW50LXNpZ25hdHVyZQo=\n' >"$1/Cull_aarch64.app.tar.gz.sig"
}
setup_preexisting_evidence() {
  mkdir -p "$2"
  printf 'old provenance\n' >"$2/release-provenance.json"
  printf 'old checksums\n' >"$2/checksums.txt"
}
setup_log_symlink() {
  local victim="$tmp_root/log-victim"
  printf 'do-not-touch\n' >"$victim"
  mkdir -p "$2"
  ln -s "$victim" "$2/verification.log"
}
setup_symlink_asset() {
  local outside="$tmp_root/outside.dmg"
  printf 'outside\n' >"$outside"
  unlink "$1/Cull_0.2.6_aarch64.dmg"
  ln -s "$outside" "$1/Cull_0.2.6_aarch64.dmg"
}
setup_hardlink_asset() {
  local outside="$tmp_root/outside-archive"
  printf 'outside\n' >"$outside"
  unlink "$1/Cull_aarch64.app.tar.gz"
  ln "$outside" "$1/Cull_aarch64.app.tar.gz"
}
setup_hardlink_evidence() {
  local victim="$tmp_root/evidence-victim"
  printf 'do-not-touch\n' >"$victim"
  mkdir -p "$2"
  ln "$victim" "$2/checksums.txt"
}

run_case valid pass setup_valid
[[ -s "$tmp_root/valid/output/release-provenance.json" ]] || fail 'valid provenance missing'
[[ -s "$tmp_root/valid/output/checksums.txt" ]] || fail 'valid checksums missing'
node - "$tmp_root/valid/output/release-provenance.json" <<'NODE'
const fs = require('node:fs');
const p = JSON.parse(fs.readFileSync(process.argv[2]));
if (p.schema !== 'cull.release.provenance.v1' || p.version !== '0.2.6' || p.tag !== 'v0.2.6') process.exit(1);
if (p.commit !== '0123456789abcdef0123456789abcdef01234567' || p.workflowRunId !== '123') process.exit(1);
if (p.tagObjectSha !== null) process.exit(1);
if (Object.keys(p.assets).length !== 4) process.exit(1);
if (!Object.values(p.checks).every(Boolean)) process.exit(1);
NODE
CASE_TAG_OBJECT_SHA=89abcdef0123456789abcdef0123456789abcdef run_case tagged-provenance pass setup_valid
node - "$tmp_root/tagged-provenance/output/release-provenance.json" <<'NODE'
const fs = require('node:fs');
const p = JSON.parse(fs.readFileSync(process.argv[2]));
if (p.tagObjectSha !== '89abcdef0123456789abcdef0123456789abcdef') process.exit(1);
NODE
CASE_TAG_OBJECT_SHA=ABC run_case invalid-tag-object-sha fail setup_valid
run_case missing-signature fail setup_missing_signature
run_case extra-asset fail setup_extra_asset
run_case stale-latest-json fail setup_stale_metadata
run_case wrong-updater-url fail setup_wrong_asset_url
run_case signature-mismatch fail setup_signature_mismatch
CASE_MINISIGN_STATUS=1 run_case failed-updater-signature fail setup_valid
CASE_BUNDLE_VERSION=0.2.5 run_case wrong-embedded-version fail setup_valid
CASE_ARCHS=x86_64 run_case wrong-architecture fail setup_valid
CASE_CODESIGN_STATUS=1 run_case failed-codesign fail setup_valid
CASE_SPCTL_STATUS=1 run_case failed-gatekeeper fail setup_valid
CASE_STAPLER_STATUS=1 run_case failed-stapler fail setup_valid
CASE_DETACH_STATUS=1 run_case failed-detach fail setup_valid
CASE_ATTACH_STATUS=1 CASE_DETACH_STATUS=1 run_case partial-attach-failure fail setup_valid
[[ -s "$tmp_root/partial-attach-failure/detach-marker" ]] || fail 'partial attach failure did not attempt detach'
if find "$tmp_root/partial-attach-failure/runner-temp" -maxdepth 1 -type d -name 'cull-release-verify.*' | grep -q .; then
  fail 'unmounted partial attach retained its private workdir'
fi
CASE_ATTACH_STATUS=1 CASE_DETACH_STATUS=1 CASE_MOUNT_ACTIVE=1 run_case active-partial-mount-retained fail setup_valid
find "$tmp_root/active-partial-mount-retained/runner-temp" -maxdepth 1 -type d -name 'cull-release-verify.*' | grep -q . || fail 'active partial mount workdir was not retained'
rg -q 'active mount.*retaining|retaining.*active mount' "$tmp_root/active-partial-mount-retained/stderr" || fail 'active mount retention was not reported'
CASE_ATTACH_STATUS=1 CASE_DETACH_STATUS=1 CASE_MOUNT_STATUS=1 run_case mount-inventory-failure-retained fail setup_valid
find "$tmp_root/mount-inventory-failure-retained/runner-temp" -maxdepth 1 -type d -name 'cull-release-verify.*' | grep -q . || fail 'mount inventory failure did not retain workdir'
rg -q 'mount inventory.*failed|failed.*mount inventory' "$tmp_root/mount-inventory-failure-retained/stderr" || fail 'mount inventory failure was not reported'
CASE_ALLOW_EXISTING_EVIDENCE=1 run_case preexisting-evidence fail setup_preexisting_evidence
[[ "$(cat "$tmp_root/preexisting-evidence/output/release-provenance.json")" == 'old provenance' ]] || fail 'preexisting provenance was modified'
[[ "$(cat "$tmp_root/preexisting-evidence/output/checksums.txt")" == 'old checksums' ]] || fail 'preexisting checksums were modified'
run_case symlink-substitution fail setup_symlink_asset
run_case hardlink-substitution fail setup_hardlink_asset
run_case output-log-symlink fail setup_log_symlink
[[ "$(cat "$tmp_root/log-victim")" == 'do-not-touch' ]] || fail 'output log symlink target was modified'
CASE_ALLOW_EXISTING_EVIDENCE=1 run_case hardlink-evidence fail setup_hardlink_evidence
[[ "$(cat "$tmp_root/evidence-victim")" == 'do-not-touch' ]] || fail 'hard-linked evidence target was modified'
CASE_MUTATE_SOURCE=1 run_case source-mutation-after-snapshot pass setup_valid
expected_archive_sha="$(printf 'archive\n' | shasum -a 256 | awk '{print $1}')"
rg -q "^${expected_archive_sha}  Cull_aarch64.app.tar.gz$" "$tmp_root/source-mutation-after-snapshot/output/checksums.txt" || fail 'evidence did not hash acquired snapshot'
CASE_ALLOW_EXISTING_EVIDENCE=1 CASE_RACE_DESTINATION_NAME=checksums.txt run_case evidence-destination-race fail setup_valid
[[ -d "$tmp_root/evidence-destination-race/output/checksums.txt" ]] || fail 'raced evidence directory was overwritten or moved into'
CASE_ALLOW_EXISTING_EVIDENCE=1 CASE_RACE_PUBLISHED_NAME=checksums.txt run_case post-rename-inode-race fail setup_valid
[[ "$(cat "$tmp_root/post-rename-inode-race/output/checksums.txt")" == 'raced replacement' ]] || fail 'post-rename replacement was overwritten or accepted'
run_case cleanup-without-trash pass setup_valid
if find "$tmp_root/cleanup-without-trash/runner-temp" -maxdepth 1 -type d -name 'cull-release-verify.*' | grep -q .; then
  fail 'fallback cleanup left invocation-owned workdir in place'
fi
find "$tmp_root/cleanup-without-trash/runner-temp" -mindepth 1 -maxdepth 1 -type d -name '.cull-cleanup-claim.*' | grep -q . || fail 'fallback cleanup did not atomically claim the private workdir'
CASE_SIGNAL_DURING_PUBLISH=SIGTERM run_case signal-during-evidence-publish fail setup_valid
[[ "$(cat "$tmp_root/signal-during-evidence-publish/status")" == 143 ]] || fail 'deferred SIGTERM did not exit 143'

helper_parent="$tmp_root/helper-races"
mkdir -p "$helper_parent/cull-private.candidate" "$helper_parent/replacement"
printf 'original\n' >"$helper_parent/cull-private.candidate/value"
printf 'replacement\n' >"$helper_parent/replacement/value"
set +e
PATH="$tmp_root/valid/bin" CULL_RELEASE_TEST_MODE=1 CULL_SAFE_CLEANUP_TEST_SWAP_WITH="$helper_parent/replacement" \
  bash "$repo_root/scripts/safe-cleanup-private.sh" "$helper_parent/cull-private.candidate" "$helper_parent" 'cull-private.'
helper_status=$?
set -e
[[ $helper_status -ne 0 ]] || fail 'basename swap cleanup race unexpectedly succeeded'
[[ "$(cat "$helper_parent/cull-private.candidate/value")" == 'replacement' ]] || fail 'basename swap replacement was not preserved'
rg --hidden -l '^original$' "$helper_parent" >/dev/null || fail 'basename swap original was not preserved in claim container'
pass_count=$((pass_count + 1))
printf 'ok %d - cleanup-basename-swap-race\n' "$pass_count"

mkdir -p "$helper_parent/cull-private.container-race"
printf 'container-original\n' >"$helper_parent/cull-private.container-race/value"
set +e
PATH="$tmp_root/valid/bin" CULL_RELEASE_TEST_MODE=1 CULL_SAFE_CLEANUP_TEST_RACE_CONTAINER=1 \
  bash "$repo_root/scripts/safe-cleanup-private.sh" "$helper_parent/cull-private.container-race" "$helper_parent" 'cull-private.'
helper_status=$?
set -e
[[ $helper_status -ne 0 ]] || fail 'container replacement race unexpectedly succeeded'
[[ "$(cat "$helper_parent/cull-private.container-race/value")" == 'container-original' ]] || fail 'container race touched candidate'
pass_count=$((pass_count + 1))
printf 'ok %d - cleanup-container-race\n' "$pass_count"

wrapper_case="$tmp_root/wrapper-cleanup"
wrapper_source="$wrapper_case/source"
wrapper_runner_temp="$wrapper_case/runner-temp"
wrapper_output="$wrapper_case/output"
wrapper_bin="$wrapper_case/bin"
mkdir -p "$wrapper_runner_temp"
make_tools "$wrapper_bin"
make_artifacts "$wrapper_source" 0.2.5
PATH="$wrapper_bin" RUNNER_TEMP="$wrapper_runner_temp" FAKE_BUNDLE_VERSION=0.2.5 \
  FAKE_MOUNT_PATH_FILE="$wrapper_case/mount-path" \
  bash "$repo_root/scripts/clean-machine-dmg-gate.sh" \
    --dmg-path "$wrapper_source/Cull_0.2.5_aarch64.dmg" \
    --archive-path "$wrapper_source/Cull_aarch64.app.tar.gz" \
    --signature-path "$wrapper_source/Cull_aarch64.app.tar.gz.sig" \
    --out-dir "$wrapper_output" >/dev/null
if find "$wrapper_runner_temp" -maxdepth 1 -type d -name 'cull-local-artifacts.*' | grep -q .; then
  fail 'clean-machine wrapper leaked its owned staging directory'
fi
pass_count=$((pass_count + 1))
printf 'ok %d - wrapper-owned-staging-cleanup\n' "$pass_count"

if rg -n '/Applications|rm -rf' "$repo_root/scripts/clean-machine-dmg-gate.sh"; then
  fail 'clean-machine gate still mutates /Applications or uses rm -rf'
fi

printf '1..%d\n' "$pass_count"
