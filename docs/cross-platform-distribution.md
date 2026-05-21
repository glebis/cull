# Cross-Platform Distribution

Status: Draft, 2026-05-13

ImageView is a Tauri 2 desktop app. The practical release path is macOS first, then Linux, then Windows after a portability pass. Android and iOS are technically supported by Tauri 2, but they are separate product tracks for this app because ImageView depends heavily on desktop filesystem access, local model files, tray/background behavior, file associations, MCP, and large-screen culling workflows.

## Current State

| Platform | Release status | Effort | Notes |
|---|---:|---:|---|
| macOS arm64 | Active | Low | Existing release workflow builds `aarch64-apple-darwin`. |
| macOS x64 | Active | Low | Existing release workflow builds `x86_64-apple-darwin`. |
| Linux x64 | Next | Medium | Unix socket MCP code is compatible. Needs distro dependency testing, packaging, and Secret Service validation. |
| Windows x64 | Planned | Medium/high | Tauri support is strong, but current Unix socket MCP code will not compile on Windows. |
| Linux arm64 | Later | High | Possible, but AppImage/ONNX/runtime testing will take extra CI work. |
| Android/iOS | Deferred | High | Requires mobile UX and filesystem model redesign. |

The current Tauri config uses `"bundle.targets": "all"`, creates updater artifacts, and points the updater at GitHub Releases:

```json
{
  "bundle": {
    "active": true,
    "createUpdaterArtifacts": true,
    "targets": "all"
  },
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/glebis/imageview/releases/latest/download/latest.json"
      ]
    }
  }
}
```

The current GitHub release workflow builds macOS Intel and Apple Silicon artifacts. Add Linux and Windows runners only after the portability checklist below passes locally or in dedicated CI branches.

## Current Publish Audit

Status: macOS-first direct-download release path, updated 2026-05-22.

Agent-complete items:

- Tauri updater artifacts are enabled with `bundle.createUpdaterArtifacts = true`, matching the configured `latest.json` endpoint and updater public key.
- The GitHub release workflow runs the frontend and Rust quality gates before packaging.
- The release workflow imports a macOS signing certificate and passes Apple notarization credentials plus Tauri updater signing credentials to `tauri-action`.
- Frontend dependencies were refreshed inside the existing semver ranges, and the remaining `cookie` advisory is pinned to `0.7.2` via npm overrides.

Human-owned release blockers:

- Apple Developer Program membership with authority to create a Developer ID Application certificate.
- GitHub Actions secrets: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`, `KEYCHAIN_PASSWORD`, `TAURI_SIGNING_PRIVATE_KEY`, and optionally `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
- Private backup of the Tauri updater signing key. Losing it means installed users cannot receive trusted updates from this update channel.
- Clean-machine smoke testing on Apple Silicon and Intel macOS before publishing the draft release.
- Final publishing decision for license and positioning: the repo is source-available under BUSL-1.1, not OSI open source.

Remaining agent-track work before expanding beyond macOS:

- Add Linux release matrix only after Linux dependency/install smoke tests pass on the oldest supported distro.
- Add Windows release matrix only after MCP local transport is abstracted away from Unix sockets and trash behavior is implemented or explicitly unsupported on Windows.
- Add artifact smoke-test automation once signed macOS artifacts exist.

## Platform Support Notes

### Linux

Linux is the easiest non-macOS target because the MCP stdio bridge and socket server use Unix domain sockets, which are available on Linux. Tauri uses WebKitGTK on Linux, so build machines need the WebKitGTK and appindicator packages installed.

Recommended first artifacts:

| Artifact | Purpose |
|---|---|
| `.AppImage` | Easiest direct-download artifact for early testers. |
| `.deb` | Better Ubuntu/Debian install and dependency handling. |
| `.rpm` | Fedora/RHEL-family users once packaging is stable. |

Build Linux artifacts on the oldest Ubuntu base we intend to support, not the newest runner by default. glibc compatibility is forward-compatible, not backward-compatible, so binaries built on a newer distro can fail on older systems.

### Windows

Windows is a good target for ImageView, but it needs a small platform abstraction before we enable the release runner.

Current blockers:

- `src-tauri/src/mcp/socket.rs` imports `tokio::net::UnixListener`.
- `src-tauri/src/lib.rs` connects to `tokio::net::UnixStream` for `--mcp-stdio`.
- `trash_images` currently uses macOS Finder AppleScript and is a no-op on other platforms.

Recommended fix:

- Keep Unix sockets for macOS/Linux.
- Add a Windows transport for local MCP, either named pipes or localhost TCP with loopback-only binding and token/permission checks.
- Make non-macOS trash behavior explicit in the UI/API: use platform trash APIs or return a clear unsupported error.

For packaging, prefer NSIS first because it produces a familiar setup executable. MSI can follow if we need enterprise deployment or Microsoft Store-related packaging.

Tauri uses Microsoft Edge WebView2 on Windows. Modern Windows usually has it already, and Tauri installers can handle WebView2 installation policy. For normal online installers, keep the default downloaded bootstrapper. For offline environments, configure the Windows `webviewInstallMode` explicitly and accept the larger installer.

### Mobile

Do not treat mobile as another release artifact. Before Android/iOS work starts, decide:

- Library model: app-private storage, system photo picker, or scoped directory access.
- Import model: one-shot picker import versus watched roots.
- MCP model: disabled, local network only, or mobile-specific.
- UI model: touch-first review flow rather than desktop command palette/grid density.
- Model/runtime model: whether local ONNX inference is acceptable on device.

## Quality Gates

Run these before any release build:

```bash
npm ci
npm run check
npm test
cd src-tauri && cargo test
```

Run browser E2E when the frontend changed or before publishing a release candidate:

```bash
"/Applications/Google Chrome Beta.app/Contents/MacOS/Google Chrome Beta" \
  --remote-debugging-port=9222 \
  --user-data-dir="$HOME/.chrome-beta-profile" &

npx vite dev --port 1420 &
bash tests/e2e/run-e2e.sh
```

Run at least one full Tauri build on every release platform:

```bash
npm run tauri build
```

For platform-specific bundles:

```bash
# macOS direct download
npm run tauri build -- --bundles app,dmg

# Linux direct download and package-manager artifacts
npm run tauri build -- --bundles appimage,deb,rpm

# Windows installer artifacts, on a Windows runner
npm run tauri build -- --bundles nsis,msi
```

## Local Build Prerequisites

### macOS

```bash
xcode-select --install
rustup target add aarch64-apple-darwin x86_64-apple-darwin
npm ci
```

Build:

```bash
npm run tauri build -- --target aarch64-apple-darwin
npm run tauri build -- --target x86_64-apple-darwin
```

Release builds for public distribution need Developer ID signing and notarization. The updater also needs Tauri updater signing keys.

### Linux

Ubuntu/Debian build dependencies:

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf
```

Build:

```bash
npm ci
npm run tauri build -- --bundles appimage,deb,rpm
```

Linux validation must cover at least Ubuntu LTS and one Fedora-family distro before calling the release generally supported.

### Windows

Install:

- Microsoft C++ Build Tools with "Desktop development with C++".
- Rust stable for Windows.
- Node.js LTS.
- WebView2 runtime if the test machine does not already have it.

Build on Windows:

```powershell
npm ci
npm run check
npm test
cd src-tauri
cargo test
cd ..
npm run tauri build -- --bundles nsis
```

Add `msi` once WiX/MSI-specific signing and install tests are in place.

## CI Release Matrix

The release workflow should move in stages:

1. Keep macOS matrix as-is.
2. Add Linux x64 with `ubuntu-22.04` or the oldest supported runner/container.
3. Add Windows x64 after MCP transport compiles on Windows.
4. Add signing/notarization secrets.
5. Add release smoke tests against produced artifacts.
6. Promote draft releases manually after install testing.

Target matrix:

```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - platform: macos-latest
        args: '--target aarch64-apple-darwin'
      - platform: macos-latest
        args: '--target x86_64-apple-darwin'
      - platform: ubuntu-22.04
        args: ''
      - platform: windows-latest
        args: ''
```

Linux runners need this step before the Tauri action:

```yaml
- name: Install Linux dependencies
  if: matrix.platform == 'ubuntu-22.04'
  run: |
    sudo apt-get update
    sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

The Tauri action can continue to create draft GitHub releases. Keep draft releases until all artifact smoke tests pass.

## Smoke Test Checklist

Run these on a clean machine or VM for each artifact:

| Area | Test |
|---|---|
| Install | Installer/app launches without terminal. |
| Database | New profile creates app data directory and SQLite DB without touching existing user DBs. |
| Import | Import a folder with JPEG, PNG, WebP, GIF, and a nested subfolder. |
| Thumbnails | Thumbnails generate and persist after restart. |
| Curation | Star rating, accept/reject/undecide, undo/redo persist after restart. |
| Collections | Create collection, add images, restart, verify contents. |
| Smart collections | Run a rating/source/orientation query. |
| Models | Download/check CLIP model; generate embeddings for a small set. |
| Search | Similarity/embedding view loads without blank canvas or UI lockup. |
| Files | Drag-and-drop files/folders where the OS supports it. |
| Open With | OS file association opens one image and multiple images. |
| Deep links | `imageview://` or `cull://` route opens the app and focuses the requested view. |
| MCP | `--mcp-stdio` connects locally; HTTP MCP works only when explicitly enabled. |
| Updater | Installed app can read `latest.json`; invalid signatures are rejected. |
| Uninstall | User library database is not deleted by uninstall. |

Platform-specific checks:

| Platform | Extra checks |
|---|---|
| macOS | Gatekeeper, notarization, Finder Open With, drag-and-drop, tray/menu behavior, Apple Silicon and Intel launch. |
| Linux | AppImage executes after `chmod +x`, `.deb` dependencies install cleanly, tray behavior works under GNOME/KDE where supported. |
| Windows | NSIS installer, uninstall, Start Menu shortcut, WebView2 install path, Defender/SmartScreen reputation, path handling with spaces/non-ASCII. |

## Release Procedure

1. Confirm version in `src-tauri/tauri.conf.json`.
2. Run quality gates.
3. Build platform artifacts in CI from a clean tag.
4. Keep GitHub release as draft.
5. Install artifacts on clean machines/VMs.
6. Run smoke checklist.
7. Confirm updater metadata and signatures.
8. Publish the GitHub release.
9. Install from the public release URL and verify the app still launches.

## Documentation Sources

- [Tauri distribute overview](https://v2.tauri.app/distribute/)
- [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
- [Tauri WebView versions](https://v2.tauri.app/reference/webview-versions/)
- [Tauri Windows installer](https://v2.tauri.app/distribute/windows-installer/)
- [Tauri Debian packaging](https://v2.tauri.app/distribute/debian/)
- [Tauri GitHub Actions pipeline](https://v2.tauri.app/distribute/pipelines/github/)
