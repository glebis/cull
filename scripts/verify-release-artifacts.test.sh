#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
verifier="$repo_root/scripts/verify-release-artifacts.sh"
tmp_root="$(mktemp -d "${TMPDIR:-/tmp}/cull-artifact-test.XXXXXX")"
trap 'trash "$tmp_root" >/dev/null 2>&1 || true' EXIT

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
  mkdir -p "$mountpoint/Cull.app/Contents/MacOS"
  : >"$mountpoint/Cull.app/Contents/Info.plist"
  : >"$mountpoint/Cull.app/Contents/MacOS/Cull"
  exit "${FAKE_ATTACH_STATUS:-0}"
fi
if [[ "$1" == "detach" ]]; then
  exit "${FAKE_DETACH_STATUS:-0}"
fi
exit 1
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
exit "${FAKE_MINISIGN_STATUS:-0}"
EOF
  chmod +x "$bin_dir"/*
}

make_artifacts() {
  local dir=$1
  local version=${2:-0.2.6}
  mkdir -p "$dir"
  printf 'dmg\n' >"$dir/Cull_${version}_aarch64.dmg"
  printf 'archive\n' >"$dir/Cull_aarch64.app.tar.gz"
  printf 'signature-value\n' >"$dir/Cull_aarch64.app.tar.gz.sig"
  node - "$dir/latest.json" "$version" <<'NODE'
const fs = require('node:fs');
const [path, version] = process.argv.slice(2);
fs.writeFileSync(path, JSON.stringify({
  version,
  notes: 'test',
  pub_date: '2026-07-10T00:00:00Z',
  platforms: {
    'darwin-aarch64': {
      signature: 'signature-value',
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
  mkdir -p "$case_dir"
  make_tools "$bin_dir"
  make_artifacts "$artifacts"
  "$setup" "$artifacts" "$output"

  set +e
  PATH="$bin_dir:$PATH" \
    FAKE_CODESIGN_STATUS="${CASE_CODESIGN_STATUS:-0}" \
    FAKE_SPCTL_STATUS="${CASE_SPCTL_STATUS:-0}" \
    FAKE_MINISIGN_STATUS="${CASE_MINISIGN_STATUS:-0}" \
    FAKE_BUNDLE_VERSION="${CASE_BUNDLE_VERSION:-0.2.6}" \
    FAKE_ARCHS="${CASE_ARCHS:-arm64}" \
    FAKE_STAPLER_STATUS="${CASE_STAPLER_STATUS:-0}" \
    FAKE_DETACH_STATUS="${CASE_DETACH_STATUS:-0}" \
    bash "$verifier" \
      --artifact-dir "$artifacts" \
      --version 0.2.6 \
      --tag v0.2.6 \
      --commit 0123456789abcdef0123456789abcdef01234567 \
      --run-id 123 \
      --out "$output" >"$case_dir/stdout" 2>"$case_dir/stderr"
  local status=$?
  set -e

  if [[ "$expected" == pass && $status -ne 0 ]]; then
    cat "$case_dir/stderr" >&2
    fail "$name expected success"
  fi
  if [[ "$expected" == fail && $status -eq 0 ]]; then
    fail "$name expected failure"
  fi
  if [[ "$expected" == fail && ( -e "$output/release-provenance.json" || -e "$output/checksums.txt" ) ]]; then
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
  printf 'different-signature\n' >"$1/Cull_aarch64.app.tar.gz.sig"
}
setup_stale_success_evidence() {
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

run_case valid pass setup_valid
[[ -s "$tmp_root/valid/output/release-provenance.json" ]] || fail 'valid provenance missing'
[[ -s "$tmp_root/valid/output/checksums.txt" ]] || fail 'valid checksums missing'
node - "$tmp_root/valid/output/release-provenance.json" <<'NODE'
const fs = require('node:fs');
const p = JSON.parse(fs.readFileSync(process.argv[2]));
if (p.schema !== 'cull.release.provenance.v1' || p.version !== '0.2.6' || p.tag !== 'v0.2.6') process.exit(1);
if (p.commit !== '0123456789abcdef0123456789abcdef01234567' || p.workflowRunId !== '123') process.exit(1);
if (Object.keys(p.assets).length !== 4) process.exit(1);
if (!Object.values(p.checks).every(Boolean)) process.exit(1);
NODE
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
CASE_STAPLER_STATUS=1 run_case stale-success-evidence fail setup_stale_success_evidence
run_case symlink-substitution fail setup_symlink_asset
run_case hardlink-substitution fail setup_hardlink_asset
run_case output-log-symlink fail setup_log_symlink
[[ "$(cat "$tmp_root/log-victim")" == 'do-not-touch' ]] || fail 'output log symlink target was modified'

if rg -n '/Applications|rm -rf' "$repo_root/scripts/clean-machine-dmg-gate.sh"; then
  fail 'clean-machine gate still mutates /Applications or uses rm -rf'
fi

printf '1..%d\n' "$pass_count"
