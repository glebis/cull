#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${BD_BIN:-}" ]]; then
  if [[ ! -x "$BD_BIN" ]]; then
    echo "BD_BIN is set but is not executable: $BD_BIN" >&2
    exit 127
  fi
  selected="$BD_BIN"
elif [[ -x /usr/local/bin/bd ]]; then
  # This repo's .beads Dolt database is currently owned by bd 1.x. On machines
  # with both binaries installed, /opt/homebrew/bin/bd may be an older 0.x build
  # with an incompatible schema (for example, missing the crystallizes column).
  selected=/usr/local/bin/bd
elif [[ -x /opt/homebrew/bin/bd ]]; then
  selected=/opt/homebrew/bin/bd
else
  selected="$(command -v bd || true)"
  if [[ -z "$selected" ]]; then
    echo "bd not found. Install beads or set BD_BIN to an executable bd binary." >&2
    exit 127
  fi
fi

echo "Using bd: $selected" >&2
exec "$selected" "$@"
