#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/clean-machine-dmg-gate.sh [--build] [--install] [--artifact-dir DIR] [--dmg-path PATH] [--archive-path PATH] [--signature-path PATH] [--out-dir DIR]

Builds or stages a local signed Cull release, then delegates all inventory,
updater, mounted-app, signature, notarization, and architecture checks to
scripts/verify-release-artifacts.sh. The optional launch copy is isolated under
$RUNNER_TEMP/install; this command never writes to the system app directory.

Options:
  --build                 Run npm run tauri build before verification.
  --install               Copy the verified mounted app to an isolated temp path and launch it.
  --artifact-dir DIR      Verify an already-normalized four-file artifact directory.
  --dmg-path PATH         Override the locally built DMG path.
  --archive-path PATH     Override the locally built updater archive path.
  --signature-path PATH   Override the updater signature path.
  --out-dir DIR           Evidence output (default: docs/release-audit-2026-06-09).
  --help                  Show this help.
USAGE
}

die() {
  printf 'clean-machine-dmg-gate: %s\n' "$*" >&2
  exit 1
}

BUILD=0
INSTALL=0
ARTIFACT_DIR=""
DMG_PATH=""
ARCHIVE_PATH=""
SIGNATURE_PATH=""
OUT_DIR="docs/release-audit-2026-06-09"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --build) BUILD=1; shift ;;
    --install) INSTALL=1; shift ;;
    --artifact-dir|--dmg-path|--archive-path|--signature-path|--out-dir)
      [[ $# -ge 2 ]] || die "missing value for $1"
      case "$1" in
        --artifact-dir) ARTIFACT_DIR=$2 ;;
        --dmg-path) DMG_PATH=$2 ;;
        --archive-path) ARCHIVE_PATH=$2 ;;
        --signature-path) SIGNATURE_PATH=$2 ;;
        --out-dir) OUT_DIR=$2 ;;
      esac
      shift 2
      ;;
    --allow-local-dev) die '--allow-local-dev was removed because release trust checks cannot be bypassed' ;;
    --app-path) die '--app-path was removed; verification always uses Cull.app mounted from the DMG' ;;
    --help|-h) usage; exit 0 ;;
    *) usage >&2; die "unknown argument: $1" ;;
  esac
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$repo_root"

if [[ $BUILD -eq 1 ]]; then
  npm run tauri build -- --target aarch64-apple-darwin
fi

version="$(node -e "process.stdout.write(require('./package.json').version)")"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die 'package.json version is not X.Y.Z'
tag="v$version"
commit="$(git rev-parse HEAD)"
run_id="${GITHUB_RUN_ID:-$(date +%s)}"

if [[ -z "$ARTIFACT_DIR" ]]; then
  dmg_name="Cull_${version}_aarch64.dmg"
  archive_name="Cull_aarch64.app.tar.gz"

  if [[ -z "$DMG_PATH" ]]; then
    for candidate in \
      "src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/$dmg_name" \
      "src-tauri/target/release/bundle/dmg/$dmg_name"; do
      if [[ -f "$candidate" && ! -L "$candidate" ]]; then
        DMG_PATH=$candidate
        break
      fi
    done
  fi
  if [[ -z "$ARCHIVE_PATH" ]]; then
    for candidate in \
      "src-tauri/target/aarch64-apple-darwin/release/bundle/macos/$archive_name" \
      "src-tauri/target/release/bundle/macos/$archive_name"; do
      if [[ -f "$candidate" && ! -L "$candidate" ]]; then
        ARCHIVE_PATH=$candidate
        break
      fi
    done
  fi
  [[ -n "$SIGNATURE_PATH" ]] || SIGNATURE_PATH="${ARCHIVE_PATH}.sig"
  for path in "$DMG_PATH" "$ARCHIVE_PATH" "$SIGNATURE_PATH"; do
    [[ -n "$path" && -f "$path" && ! -L "$path" ]] || die "local signed artifact not found or unsafe: ${path:-<unset>}"
  done

  staging_root="$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/cull-local-artifacts.${run_id}.XXXXXX")"
  ARTIFACT_DIR="$staging_root/artifacts"
  mkdir "$ARTIFACT_DIR"
  cp "$DMG_PATH" "$ARTIFACT_DIR/$dmg_name"
  cp "$ARCHIVE_PATH" "$ARTIFACT_DIR/$archive_name"
  cp "$SIGNATURE_PATH" "$ARTIFACT_DIR/$archive_name.sig"
  node - "$ARTIFACT_DIR/latest.json" "$version" "$tag" "$ARTIFACT_DIR/$archive_name.sig" <<'NODE'
const fs = require('node:fs');
const [output, version, tag, signaturePath] = process.argv.slice(2);
const signature = fs.readFileSync(signaturePath, 'utf8').trim();
const metadata = {
  version,
  notes: 'Local clean-machine verification',
  pub_date: '1970-01-01T00:00:00Z',
  platforms: {
    'darwin-aarch64': {
      signature,
      url: `https://github.com/glebis/cull/releases/download/${tag}/Cull_aarch64.app.tar.gz`
    }
  }
};
fs.writeFileSync(output, JSON.stringify(metadata, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
NODE
fi

args=(
  --artifact-dir "$ARTIFACT_DIR"
  --version "$version"
  --tag "$tag"
  --commit "$commit"
  --run-id "$run_id"
  --out "$OUT_DIR"
)
[[ $INSTALL -eq 0 ]] || args+=(--launch)

exec "$repo_root/scripts/verify-release-artifacts.sh" "${args[@]}"
