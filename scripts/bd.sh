#!/usr/bin/env bash
set -euo pipefail

if [[ -n "${BD_BIN:-}" ]]; then
  if [[ ! -x "$BD_BIN" ]]; then
    echo "BD_BIN is set but is not executable: $BD_BIN" >&2
    exit 127
  fi
  selected="$BD_BIN"
elif [[ -x /opt/homebrew/bin/bd ]]; then
  selected=/opt/homebrew/bin/bd
elif [[ -x /usr/local/bin/bd ]]; then
  selected=/usr/local/bin/bd
else
  selected="$(command -v bd || true)"
  if [[ -z "$selected" ]]; then
    echo "bd not found. Install beads or set BD_BIN to an executable bd binary." >&2
    exit 127
  fi
fi

echo "Using bd: $selected" >&2
exec "$selected" "$@"
