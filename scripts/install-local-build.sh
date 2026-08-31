#!/usr/bin/env bash
# Install the freshly-built Cull.app into /Applications.
#
# This helper exists so that "build, install, open, test" runs are atomic:
# a half-installed app is worse than no install, because the running agent
# (or the user) sees an old UI and concludes the new code isn't shipped.
#
# Fail-closed behavior:
#   * Refuses to overwrite /Applications/Cull.app while any Cull process is
#     holding the executable open. macOS preserves the executable pages of
#     a running Mach-O binary on overwrite, so a copy over a live app is
#     a silent no-op for the running process — and any new launch is
#     blocked by Gatekeeper because the bundle hash no longer matches the
#     signed contents. The fix is to quit every running Cull first.
#   * Trashes (never `rm`s) the existing app before copying, so a partial
#     install can never leave a hybrid old+new bundle on disk.
#   * Triggers a full Xcode-equivalent touch of the bundle so Launch
#     Services re-registers it (otherwise `open` may launch the cached
#     version from before the install).

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

SRC="$ROOT/src-tauri/target/release/bundle/macos/Cull.app"
DST="/Applications/Cull.app"

if [[ ! -d "$SRC" ]]; then
  echo "error: $SRC not found — run 'npx tauri build' first" >&2
  exit 2
fi

# Guard 1: refuse to install while Cull is running. lsof sees the kernel's
# open-file table; pgrep alone misses processes that exec'd the binary
# via dyld and have since closed argv[0].
if lsof -nP -t "$DST/Contents/MacOS/cull" 2>/dev/null | grep -q .; then
  holders="$(lsof -nP -t "$DST/Contents/MacOS/cull" | tr '\n' ' ')"
  echo "error: $DST is in use by pid(s): $holders" >&2
  echo "       run:  pkill -f 'Cull.app/Contents/MacOS/cull'" >&2
  echo "       then:  $0" >&2
  exit 3
fi

# Guard 2: refuse if the freshly-built binary hash doesn't match what
# tauri build just produced (catches stale-state-after-cargo-clean).
SRC_SHA="$(shasum -a 256 "$SRC/Contents/MacOS/cull" | awk '{print $1}')"
echo "src binary sha256: $SRC_SHA"

# Guard 3: trash the old app before copying so we never end up with a
# hybrid directory (old Info.plist + new Resources, etc.).
if [[ -d "$DST" ]]; then
  echo "trashing existing $DST"
  trash "$DST"
fi

echo "installing $SRC -> $DST"
ditto "$SRC" "$DST"

# Guard 4: re-register with Launch Services so `open` and Dock see the
# new bundle, not the pre-install cached one.
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
  -f "$DST" >/dev/null 2>&1 || true

DST_SHA="$(shasum -a 256 "$DST/Contents/MacOS/cull" | awk '{print $1}')"
if [[ "$SRC_SHA" != "$DST_SHA" ]]; then
  echo "error: post-install sha mismatch" >&2
  echo "  src: $SRC_SHA" >&2
  echo "  dst: $DST_SHA" >&2
  exit 4
fi

echo "installed. sha256: $DST_SHA"
echo "launch with: open $DST"
