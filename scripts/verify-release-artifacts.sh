#!/usr/bin/env bash
set -euo pipefail
umask 077

usage() {
  cat >&2 <<'USAGE'
Usage: scripts/verify-release-artifacts.sh --artifact-dir DIR --version X.Y.Z --tag vX.Y.Z --commit SHA --run-id ID --out DIR [--launch]
USAGE
}

die() {
  printf 'artifact verification failed: %s\n' "$*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

ARTIFACT_DIR=""
VERSION=""
TAG=""
COMMIT=""
RUN_ID=""
OUT_DIR=""
LAUNCH=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --artifact-dir|--version|--tag|--commit|--run-id|--out)
      [[ $# -ge 2 ]] || die "missing value for $1"
      case "$1" in
        --artifact-dir) ARTIFACT_DIR=$2 ;;
        --version) VERSION=$2 ;;
        --tag) TAG=$2 ;;
        --commit) COMMIT=$2 ;;
        --run-id) RUN_ID=$2 ;;
        --out) OUT_DIR=$2 ;;
      esac
      shift 2
      ;;
    --launch)
      LAUNCH=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage
      die "unknown argument: $1"
      ;;
  esac
done

[[ -n "$ARTIFACT_DIR" && -n "$VERSION" && -n "$TAG" && -n "$COMMIT" && -n "$RUN_ID" && -n "$OUT_DIR" ]] || {
  usage
  die 'all required arguments must be supplied'
}
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die 'version must be X.Y.Z'
[[ "$TAG" == "v$VERSION" ]] || die "tag must equal v$VERSION"
[[ "$COMMIT" =~ ^[0-9a-f]{40}$ ]] || die 'commit must be a lowercase 40-character Git SHA'
[[ "$RUN_ID" =~ ^[0-9]+$ ]] || die 'run-id must contain decimal digits only'
[[ -d "$ARTIFACT_DIR" && ! -L "$ARTIFACT_DIR" ]] || die 'artifact-dir must be a real directory, not a symlink'

ARTIFACT_DIR="$(cd "$ARTIFACT_DIR" && pwd -P)"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"

dmg_name="Cull_${VERSION}_aarch64.dmg"
archive_name="Cull_aarch64.app.tar.gz"
signature_name="${archive_name}.sig"
metadata_name="latest.json"
required_names=("$dmg_name" "$archive_name" "$signature_name" "$metadata_name")

shopt -s nullglob
entries=("$ARTIFACT_DIR"/* "$ARTIFACT_DIR"/.[!.]* "$ARTIFACT_DIR"/..?*)
shopt -u nullglob
[[ ${#entries[@]} -eq ${#required_names[@]} ]] || die "artifact inventory must contain exactly ${#required_names[@]} files"

link_count() {
  local path=$1
  if stat -f '%l' "$path" >/dev/null 2>&1; then
    stat -f '%l' "$path"
  else
    stat -c '%h' "$path"
  fi
}

for name in "${required_names[@]}"; do
  path="$ARTIFACT_DIR/$name"
  [[ -f "$path" && ! -L "$path" ]] || die "required artifact is missing or not a regular file: $name"
  [[ "$(link_count "$path")" == 1 ]] || die "artifact must not be hard-linked: $name"
done
for entry in "${entries[@]}"; do
  base=${entry##*/}
  found=0
  for name in "${required_names[@]}"; do
    [[ "$base" == "$name" ]] && found=1
  done
  [[ $found -eq 1 ]] || die "unexpected artifact: $base"
done

mkdir -p "$OUT_DIR"
[[ -d "$OUT_DIR" && ! -L "$OUT_DIR" ]] || die 'out must be a real directory, not a symlink'
OUT_DIR="$(cd "$OUT_DIR" && pwd -P)"
case "$OUT_DIR/" in
  "$ARTIFACT_DIR/"*) die 'out must not be inside artifact-dir' ;;
esac

for stale in release-provenance.json checksums.txt; do
  if [[ -d "$OUT_DIR/$stale" && ! -L "$OUT_DIR/$stale" ]]; then
    die "cannot replace output directory: $stale"
  fi
  [[ ! -e "$OUT_DIR/$stale" && ! -L "$OUT_DIR/$stale" ]] || unlink "$OUT_DIR/$stale"
done
if [[ -L "$OUT_DIR/verification.log" ]]; then
  die 'verification.log must not be a symlink'
fi
if [[ -d "$OUT_DIR/verification.log" ]]; then
  die 'verification.log must not be a directory'
fi
[[ ! -e "$OUT_DIR/verification.log" ]] || unlink "$OUT_DIR/verification.log"

for command_name in node shasum codesign spctl xcrun hdiutil plutil lipo minisign; do
  require_command "$command_name"
done
if [[ $LAUNCH -eq 1 ]]; then
  require_command ditto
  require_command open
  [[ -n "${RUNNER_TEMP:-}" ]] || die '--launch requires RUNNER_TEMP'
fi

work_dir="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/cull-release-verify.${RUN_ID}.XXXXXX")"
mount_dir="$work_dir/mount"
mkdir "$mount_dir"
log_file="$OUT_DIR/verification.log"
set -o noclobber
: >"$log_file"
set +o noclobber
mounted=0
verification_complete=0

cleanup() {
  if [[ $mounted -eq 1 ]]; then
    hdiutil detach "$mount_dir" -quiet >>"$log_file" 2>&1 || true
    mounted=0
  fi
  rmdir "$mount_dir" >/dev/null 2>&1 || true
  rmdir "$work_dir" >/dev/null 2>&1 || true
  if [[ $verification_complete -ne 1 ]]; then
    [[ ! -e "$OUT_DIR/release-provenance.json" && ! -L "$OUT_DIR/release-provenance.json" ]] || unlink "$OUT_DIR/release-provenance.json" >/dev/null 2>&1 || true
    [[ ! -e "$OUT_DIR/checksums.txt" && ! -L "$OUT_DIR/checksums.txt" ]] || unlink "$OUT_DIR/checksums.txt" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

run_logged() {
  local label=$1
  shift
  printf '== %s ==\n' "$label" >>"$log_file"
  "$@" >>"$log_file" 2>&1 || die "$label check failed"
}

node - "$ARTIFACT_DIR/$metadata_name" "$VERSION" "$TAG" "$archive_name" "$ARTIFACT_DIR/$signature_name" "$repo_root/src-tauri/tauri.conf.json" <<'NODE' || die 'latest.json does not exactly bind the verified updater asset'
const fs = require('node:fs');
const [metadataPath, version, tag, archiveName, signaturePath, configPath] = process.argv.slice(2);
let metadata;
try {
  metadata = JSON.parse(fs.readFileSync(metadataPath, 'utf8'));
} catch {
  process.exit(1);
}
if (!metadata || metadata.version !== version || !metadata.platforms || typeof metadata.platforms !== 'object') process.exit(1);
const platformNames = Object.keys(metadata.platforms);
if (platformNames.length !== 1 || platformNames[0] !== 'darwin-aarch64') process.exit(1);
const entry = metadata.platforms['darwin-aarch64'];
if (!entry || typeof entry.url !== 'string' || typeof entry.signature !== 'string') process.exit(1);
let url;
try { url = new URL(entry.url); } catch { process.exit(1); }
const pathParts = url.pathname.split('/').filter(Boolean).map(decodeURIComponent);
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
const endpoint = new URL(config?.plugins?.updater?.endpoints?.[0]);
const endpointParts = endpoint.pathname.split('/').filter(Boolean).map(decodeURIComponent);
const releasesIndex = endpointParts.lastIndexOf('releases');
if (releasesIndex < 2 || endpoint.protocol !== 'https:') process.exit(1);
const expectedParts = [
  ...endpointParts.slice(0, releasesIndex),
  'releases', 'download', tag, archiveName
];
if (url.protocol !== endpoint.protocol || url.host !== endpoint.host) process.exit(1);
if (pathParts.length !== expectedParts.length || pathParts.some((part, index) => part !== expectedParts[index])) process.exit(1);
const detachedSignature = fs.readFileSync(signaturePath, 'utf8').trim();
if (!detachedSignature || entry.signature.trim() !== detachedSignature) process.exit(1);
NODE

node - "$repo_root/src-tauri/tauri.conf.json" "$work_dir/updater.pub" <<'NODE' || die 'could not load configured Tauri updater public key'
const fs = require('node:fs');
const [configPath, outputPath] = process.argv.slice(2);
const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
const encoded = config?.plugins?.updater?.pubkey;
if (typeof encoded !== 'string' || !encoded) process.exit(1);
const decoded = Buffer.from(encoded, 'base64').toString('utf8');
if (!decoded.includes('minisign public key') || !/^RW[QRT][A-Za-z0-9+/=]+$/m.test(decoded)) process.exit(1);
fs.writeFileSync(outputPath, decoded, { mode: 0o600, flag: 'wx' });
NODE
run_logged updater-signature minisign -Vm "$ARTIFACT_DIR/$archive_name" -x "$ARTIFACT_DIR/$signature_name" -p "$work_dir/updater.pub"

run_logged dmg-attach hdiutil attach -readonly -nobrowse -mountpoint "$mount_dir" "$ARTIFACT_DIR/$dmg_name"
mounted=1
app_path="$mount_dir/Cull.app"
[[ -d "$app_path" && ! -L "$app_path" ]] || die 'mounted DMG must contain Cull.app at its root'
info_plist="$app_path/Contents/Info.plist"
[[ -f "$info_plist" && ! -L "$info_plist" ]] || die 'mounted app is missing Contents/Info.plist'

bundle_version="$(plutil -extract CFBundleShortVersionString raw -o - "$info_plist")" || die 'could not read embedded app version'
[[ "$bundle_version" == "$VERSION" ]] || die "embedded app version is $bundle_version, expected $VERSION"
bundle_executable="$(plutil -extract CFBundleExecutable raw -o - "$info_plist")" || die 'could not read bundle executable name'
[[ "$bundle_executable" =~ ^[A-Za-z0-9._+-]+$ ]] || die 'bundle executable name is unsafe'
executable_path="$app_path/Contents/MacOS/$bundle_executable"
[[ -f "$executable_path" && ! -L "$executable_path" ]] || die 'bundle executable is missing or is a symlink'
architectures="$(lipo -archs "$executable_path")" || die 'could not inspect bundle architecture'
[[ "$architectures" == 'arm64' ]] || die "mounted app must be arm64-only: $architectures"

run_logged codesign codesign --verify --deep --strict --verbose=2 "$app_path"
run_logged gatekeeper spctl --assess --type execute --verbose=4 "$app_path"
run_logged stapler xcrun stapler validate "$app_path"

if [[ $LAUNCH -eq 1 ]]; then
  install_root="$RUNNER_TEMP/install"
  install_app="$install_root/Cull.app"
  mkdir -p "$install_root"
  [[ ! -e "$install_app" && ! -L "$install_app" ]] || die "isolated launch destination already exists: $install_app"
  run_logged install-copy ditto --rsrc "$app_path" "$install_app"
  run_logged launch open -n "$install_app"
fi

run_logged dmg-detach hdiutil detach "$mount_dir" -quiet
mounted=0

declare -a asset_shas=()
declare -a asset_sizes=()
: >"$work_dir/checksums.txt"
for name in "${required_names[@]}"; do
  sha="$(shasum -a 256 "$ARTIFACT_DIR/$name" | awk '{print $1}')"
  [[ "$sha" =~ ^[0-9a-f]{64}$ ]] || die "could not compute SHA-256 for $name"
  size="$(wc -c <"$ARTIFACT_DIR/$name" | tr -d '[:space:]')"
  [[ "$size" =~ ^[0-9]+$ ]] || die "could not compute size for $name"
  asset_shas+=("$sha")
  asset_sizes+=("$size")
  printf '%s  %s\n' "$sha" "$name" >>"$work_dir/checksums.txt"
done

node - "$work_dir/release-provenance.json" "$VERSION" "$TAG" "$COMMIT" "$RUN_ID" \
  "$dmg_name" "${asset_shas[0]}" "${asset_sizes[0]}" \
  "$archive_name" "${asset_shas[1]}" "${asset_sizes[1]}" \
  "$signature_name" "${asset_shas[2]}" "${asset_sizes[2]}" \
  "$metadata_name" "${asset_shas[3]}" "${asset_sizes[3]}" <<'NODE'
const fs = require('node:fs');
const [output, version, tag, commit, workflowRunId, ...assetFields] = process.argv.slice(2);
const assets = {};
for (let i = 0; i < assetFields.length; i += 3) {
  assets[assetFields[i]] = { sha256: assetFields[i + 1], size: Number(assetFields[i + 2]) };
}
const provenance = {
  schema: 'cull.release.provenance.v1',
  version,
  tag,
  commit,
  workflowRunId,
  assets,
  checks: {
    exactInventory: true,
    updaterMetadata: true,
    updaterSignature: true,
    dmgMountedReadOnly: true,
    embeddedVersion: true,
    arm64Only: true,
    codeSignature: true,
    gatekeeper: true,
    stapledNotarization: true
  }
};
fs.writeFileSync(output, JSON.stringify(provenance, null, 2) + '\n', { mode: 0o600, flag: 'wx' });
NODE

mv "$work_dir/checksums.txt" "$OUT_DIR/checksums.txt"
mv "$work_dir/release-provenance.json" "$OUT_DIR/release-provenance.json"
verification_complete=1
printf 'artifact verification passed for %s (%s)\n' "$TAG" "$COMMIT"
