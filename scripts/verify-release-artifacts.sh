#!/usr/bin/env bash
set -euo pipefail
umask 077

usage() {
  cat >&2 <<'USAGE'
Usage: scripts/verify-release-artifacts.sh --artifact-dir DIR --version X.Y.Z --tag vX.Y.Z --commit SHA [--tag-object-sha SHA] --run-id ID --out DIR [--launch]
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
TAG_OBJECT_SHA=""
TAG_OBJECT_SHA_SET=0
RUN_ID=""
OUT_DIR=""
LAUNCH=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --artifact-dir|--version|--tag|--commit|--tag-object-sha|--run-id|--out)
      [[ $# -ge 2 ]] || die "missing value for $1"
      case "$1" in
        --artifact-dir) ARTIFACT_DIR=$2 ;;
        --version) VERSION=$2 ;;
        --tag) TAG=$2 ;;
        --commit) COMMIT=$2 ;;
        --tag-object-sha)
          [[ $TAG_OBJECT_SHA_SET -eq 0 ]] || die 'tag-object-sha may be supplied only once'
          TAG_OBJECT_SHA=$2
          TAG_OBJECT_SHA_SET=1
          ;;
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
if [[ $TAG_OBJECT_SHA_SET -eq 1 ]]; then
  [[ "$TAG_OBJECT_SHA" =~ ^[0-9a-f]{40}$ ]] || die 'tag-object-sha must be a lowercase 40-character Git SHA'
fi
[[ "$RUN_ID" =~ ^[0-9]+$ ]] || die 'run-id must contain decimal digits only'
[[ -d "$ARTIFACT_DIR" && ! -L "$ARTIFACT_DIR" ]] || die 'artifact-dir must be a real directory, not a symlink'

ARTIFACT_DIR="$(cd "$ARTIFACT_DIR" && pwd -P)"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
# shellcheck source=scripts/safe-cleanup-private.sh
source "$repo_root/scripts/safe-cleanup-private.sh"

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

node - "$OUT_DIR" <<'NODE' || die 'output directory or evidence destinations are unsafe'
const fs = require('node:fs');
const out = process.argv[2];
const outStat = fs.lstatSync(out, { bigint: true });
if (!outStat.isDirectory() || outStat.isSymbolicLink()) process.exit(1);
if ((outStat.mode & 0o022n) !== 0n) process.exit(1);
for (const name of ['release-provenance.json', 'checksums.txt', 'verification.log']) {
  try {
    fs.lstatSync(`${out}/${name}`);
    process.exit(1);
  } catch (error) {
    if (error.code !== 'ENOENT') process.exit(1);
  }
}
NODE

for command_name in node shasum codesign spctl xcrun hdiutil plutil lipo minisign mount; do
  require_command "$command_name"
done
if [[ $LAUNCH -eq 1 ]]; then
  require_command ditto
  require_command open
  [[ -n "${RUNNER_TEMP:-}" ]] || die '--launch requires RUNNER_TEMP'
fi

work_dir="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/cull-release-verify.${RUN_ID}.XXXXXX")"
snapshot_dir="$work_dir/artifacts"
mount_dir="$work_dir/mount"
mkdir "$snapshot_dir" "$mount_dir"
evidence_stage="$(mktemp -d "$OUT_DIR/.cull-release-evidence.${RUN_ID}.XXXXXX")"
log_file="$evidence_stage/verification.log"
: >"$log_file"
attach_attempted=0
verification_complete=0
published_manifest="$evidence_stage/.published-manifest.json"

cleanup() {
  if [[ $attach_attempted -eq 1 ]]; then
    if hdiutil detach "$mount_dir" -quiet >>"$log_file" 2>&1; then
      attach_attempted=0
    else
      local mount_inventory=""
      if mount_inventory="$(mount 2>>"$log_file")"; then
        if grep -F " on $mount_dir (" <<<"$mount_inventory" >/dev/null 2>&1; then
          printf 'artifact verification cleanup failed: active mount remains at %s; retaining private workdir %s\n' "$mount_dir" "$work_dir" >&2
        else
          printf 'artifact verification cleanup: detach returned nonzero but successful mount inventory proves no active mount remains at %s\n' "$mount_dir" >&2
          attach_attempted=0
        fi
      else
        printf 'artifact verification cleanup failed: mount inventory command failed; retaining private workdir %s\n' "$work_dir" >&2
      fi
    fi
  fi
  if [[ $verification_complete -ne 1 && -f "$published_manifest" ]]; then
    node - "$published_manifest" "$evidence_stage" <<'NODE' >/dev/null 2>&1 || true
const fs = require('node:fs');
const [manifestPath, stageDir] = process.argv.slice(2);
for (const item of JSON.parse(fs.readFileSync(manifestPath, 'utf8')).reverse()) {
  try {
    const stat = fs.lstatSync(item.destination, { bigint: true });
    if (stat.isFile() && !stat.isSymbolicLink() && stat.nlink === 1n &&
        String(stat.dev) === item.dev && String(stat.ino) === item.ino) {
      fs.renameSync(item.destination, `${stageDir}/${item.name}`);
    }
  } catch {}
}
NODE
  fi
  if [[ -d "$evidence_stage" ]] && ! safe_cleanup_private "$evidence_stage" "$OUT_DIR" '.cull-release-evidence.'; then
    printf 'artifact verification cleanup failed: could not safely retire evidence staging %s\n' "$evidence_stage" >&2
  fi
  if [[ $attach_attempted -eq 0 && -d "$work_dir" ]]; then
    if ! safe_cleanup_private "$work_dir" "$(dirname "$work_dir")" 'cull-release-verify.'; then
      printf 'artifact verification cleanup failed: could not safely retire private workdir %s\n' "$work_dir" >&2
    fi
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

node - "$ARTIFACT_DIR" "$snapshot_dir" "${required_names[@]}" <<'NODE' || die 'artifact changed or became unsafe while acquiring immutable snapshots'
const fs = require('node:fs');
const path = require('node:path');
const [sourceDir, snapshotDir, ...names] = process.argv.slice(2);
const { O_RDONLY, O_WRONLY, O_CREAT, O_EXCL, O_NOFOLLOW } = fs.constants;
const sameIdentity = (a, b) => a.dev === b.dev && a.ino === b.ino;
const sameStableMetadata = (a, b) => sameIdentity(a, b) && a.size === b.size &&
  a.mtimeNs === b.mtimeNs && a.ctimeNs === b.ctimeNs && a.nlink === b.nlink;
for (const name of names) {
  const source = path.join(sourceDir, name);
  const destination = path.join(snapshotDir, name);
  let sourceFd;
  let destinationFd;
  try {
    const beforePath = fs.lstatSync(source, { bigint: true });
    if (!beforePath.isFile() || beforePath.isSymbolicLink() || beforePath.nlink !== 1n) throw new Error('unsafe source');
    sourceFd = fs.openSync(source, O_RDONLY | O_NOFOLLOW);
    const beforeFd = fs.fstatSync(sourceFd, { bigint: true });
    if (!beforeFd.isFile() || beforeFd.nlink !== 1n || !sameStableMetadata(beforePath, beforeFd)) throw new Error('source race');
    destinationFd = fs.openSync(destination, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, 0o600);
    const buffer = Buffer.allocUnsafe(1024 * 1024);
    let position = 0;
    for (;;) {
      const count = fs.readSync(sourceFd, buffer, 0, buffer.length, position);
      if (count === 0) break;
      let written = 0;
      while (written < count) written += fs.writeSync(destinationFd, buffer, written, count - written);
      position += count;
    }
    fs.fsyncSync(destinationFd);
    const afterFd = fs.fstatSync(sourceFd, { bigint: true });
    const afterPath = fs.lstatSync(source, { bigint: true });
    const snapshot = fs.fstatSync(destinationFd, { bigint: true });
    if (!sameStableMetadata(beforeFd, afterFd) || !sameStableMetadata(afterFd, afterPath)) throw new Error('source mutated');
    if (!snapshot.isFile() || snapshot.nlink !== 1n || snapshot.size !== afterFd.size) throw new Error('bad snapshot');
  } catch {
    process.exit(1);
  } finally {
    if (destinationFd !== undefined) fs.closeSync(destinationFd);
    if (sourceFd !== undefined) fs.closeSync(sourceFd);
  }
}
NODE

node - "$snapshot_dir/$metadata_name" "$VERSION" "$TAG" "$archive_name" "$snapshot_dir/$signature_name" "$repo_root/src-tauri/tauri.conf.json" "$work_dir/updater.sig" <<'NODE' || die 'latest.json or updater signature encoding is invalid'
const fs = require('node:fs');
const [metadataPath, version, tag, archiveName, signaturePath, configPath, decodedSignaturePath] = process.argv.slice(2);
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
if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(detachedSignature)) process.exit(1);
const decodedBuffer = Buffer.from(detachedSignature, 'base64');
if (decodedBuffer.toString('base64') !== detachedSignature) process.exit(1);
const decodedSignature = decodedBuffer.toString('utf8');
if (!decodedSignature.startsWith('untrusted comment: signature from tauri secret key\n')) process.exit(1);
if (!decodedSignature.includes('\ntrusted comment:')) process.exit(1);
fs.writeFileSync(decodedSignaturePath, decodedSignature, { mode: 0o600, flag: 'wx' });
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
run_logged updater-signature minisign -Vm "$snapshot_dir/$archive_name" -x "$work_dir/updater.sig" -p "$work_dir/updater.pub"

attach_attempted=1
run_logged dmg-attach hdiutil attach -readonly -nobrowse -mountpoint "$mount_dir" "$snapshot_dir/$dmg_name"
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
attach_attempted=0

declare -a asset_shas=()
declare -a asset_sizes=()
: >"$evidence_stage/checksums.txt"
for name in "${required_names[@]}"; do
  sha="$(shasum -a 256 "$snapshot_dir/$name" | awk '{print $1}')"
  [[ "$sha" =~ ^[0-9a-f]{64}$ ]] || die "could not compute SHA-256 for $name"
  size="$(wc -c <"$snapshot_dir/$name" | tr -d '[:space:]')"
  [[ "$size" =~ ^[0-9]+$ ]] || die "could not compute size for $name"
  asset_shas+=("$sha")
  asset_sizes+=("$size")
  printf '%s  %s\n' "$sha" "$name" >>"$evidence_stage/checksums.txt"
done

node - "$evidence_stage/release-provenance.json" "$VERSION" "$TAG" "$COMMIT" "$TAG_OBJECT_SHA" "$RUN_ID" \
  "$dmg_name" "${asset_shas[0]}" "${asset_sizes[0]}" \
  "$archive_name" "${asset_shas[1]}" "${asset_sizes[1]}" \
  "$signature_name" "${asset_shas[2]}" "${asset_sizes[2]}" \
  "$metadata_name" "${asset_shas[3]}" "${asset_sizes[3]}" <<'NODE'
const fs = require('node:fs');
const [output, version, tag, commit, tagObjectSha, workflowRunId, ...assetFields] = process.argv.slice(2);
const assets = {};
for (let i = 0; i < assetFields.length; i += 3) {
  assets[assetFields[i]] = { sha256: assetFields[i + 1], size: Number(assetFields[i + 2]) };
}
const provenance = {
  schema: 'cull.release.provenance.v1',
  version,
  tag,
  commit,
  tagObjectSha: tagObjectSha || null,
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

safe_cleanup_private "$work_dir" "$(dirname "$work_dir")" 'cull-release-verify.'

publish_ok=0
publish_node_ok=0
pending_signal=0
trap 'pending_signal=130' INT
trap 'pending_signal=143' TERM
if node - "$evidence_stage" "$OUT_DIR" "$published_manifest" <<'NODE'
const fs = require('node:fs');
const path = require('node:path');
const [stageDir, outDir, manifestPath] = process.argv.slice(2);
const names = ['checksums.txt', 'release-provenance.json', 'verification.log'];
const testMode = process.env.CULL_RELEASE_TEST_MODE === '1';
const raceName = testMode ? process.env.CULL_VERIFY_TEST_RACE_PUBLISHED_NAME : '';
const signalName = testMode ? process.env.CULL_VERIFY_TEST_SIGNAL_DURING_PUBLISH : '';
const raceDestinationName = testMode ? process.env.CULL_VERIFY_TEST_RACE_DESTINATION_NAME : '';
const reservations = [];
const published = [];
const identity = stat => ({ dev: String(stat.dev), ino: String(stat.ino) });
const matches = (stat, expected) => stat.isFile() && !stat.isSymbolicLink() && stat.nlink === 1n &&
  String(stat.dev) === expected.dev && String(stat.ino) === expected.ino;
try {
  const outStat = fs.lstatSync(outDir, { bigint: true });
  if (raceDestinationName) fs.mkdirSync(path.join(outDir, raceDestinationName));
  for (const name of names) {
    const source = path.join(stageDir, name);
    const sourceStat = fs.lstatSync(source, { bigint: true });
    if (!sourceStat.isFile() || sourceStat.isSymbolicLink() || sourceStat.nlink !== 1n || sourceStat.dev !== outStat.dev) throw new Error('unsafe staged evidence');
    const destination = path.join(outDir, name);
    try { fs.lstatSync(destination); throw new Error('destination exists'); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    const fd = fs.openSync(destination, fs.constants.O_WRONLY | fs.constants.O_CREAT | fs.constants.O_EXCL | fs.constants.O_NOFOLLOW, 0o600);
    const reserved = fs.fstatSync(fd, { bigint: true });
    fs.closeSync(fd);
    if (!reserved.isFile() || reserved.nlink !== 1n) throw new Error('unsafe reservation');
    reservations.push({ name, source, destination, reservation: identity(reserved), sourceIdentity: identity(sourceStat), consumed: false });
  }
  for (const item of reservations) {
    const current = fs.lstatSync(item.destination, { bigint: true });
    if (!matches(current, item.reservation)) throw new Error('reservation raced');
    fs.renameSync(item.source, item.destination);
    item.consumed = true;
    if (raceName === item.name) {
      fs.renameSync(item.destination, path.join(stageDir, `.original-${item.name}`));
      fs.writeFileSync(item.destination, 'raced replacement\n', { flag: 'wx', mode: 0o600 });
    }
    if (signalName && published.length === 0) process.kill(process.ppid, signalName);
    const finalStat = fs.lstatSync(item.destination, { bigint: true });
    if (!matches(finalStat, item.sourceIdentity)) throw new Error('published inode differs from staged evidence');
    published.push({ name: item.name, destination: item.destination, ...identity(finalStat) });
  }
  fs.writeFileSync(manifestPath, JSON.stringify(published), { flag: 'wx', mode: 0o600 });
} catch (error) {
  for (const item of [...published].reverse()) {
    try {
      const current = fs.lstatSync(item.destination, { bigint: true });
      if (matches(current, item)) fs.renameSync(item.destination, path.join(stageDir, item.name));
    } catch {}
  }
  for (const item of reservations.filter(item => !item.consumed)) {
    try {
      const current = fs.lstatSync(item.destination, { bigint: true });
      if (matches(current, item.reservation)) fs.renameSync(item.destination, path.join(stageDir, `.reservation-${item.name}`));
    } catch {}
  }
  process.exit(1);
}
NODE
then
  publish_node_ok=1
fi
if [[ $publish_node_ok -eq 1 && $pending_signal -eq 0 ]]; then
  verification_complete=1
  publish_ok=1
fi
trap 'exit 130' INT
trap 'exit 143' TERM
if [[ $pending_signal -ne 0 ]]; then
  printf 'artifact verification cancelled after deferred signal; rolling back published evidence\n' >&2
  exit "$pending_signal"
fi
if [[ $publish_ok -eq 1 ]] && ! safe_cleanup_private "$evidence_stage" "$OUT_DIR" '.cull-release-evidence.'; then
  printf 'artifact verification cleanup failed: claimed evidence staging was retained\n' >&2
fi
[[ $publish_ok -eq 1 ]] || die 'evidence destinations changed, cleanup failed, or atomic publication failed'
printf 'artifact verification passed for %s (%s)\n' "$TAG" "$COMMIT"
