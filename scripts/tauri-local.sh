#!/usr/bin/env bash
set -euo pipefail

default_key_path="$HOME/.tauri/cull-updater.key"
keychain_service="cull-tauri-updater-key"

if [ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ] && [ -z "${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ] && [ -f "$default_key_path" ]; then
  export TAURI_SIGNING_PRIVATE_KEY="$(cat "$default_key_path")"
fi

if { [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ] || [ -n "${TAURI_SIGNING_PRIVATE_KEY_PATH:-}" ]; } && [ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD+x}" ] && command -v security >/dev/null 2>&1; then
  key_password="$(security find-generic-password -a "$USER" -s "$keychain_service" -w 2>/dev/null || true)"
  if [ -n "$key_password" ]; then
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$key_password"
  fi
fi

exec tauri "$@"
