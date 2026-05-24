#!/usr/bin/env bash
set -euo pipefail

export CI=true

target="${1:-all}"

run_frontend() {
  npm ci
  npm run check
  npm test
}

run_rust() {
  (
    cd src-tauri
    cargo fmt --all -- --check
    cargo clippy --all-targets
    cargo test --all-targets
  )
}

case "$target" in
  all)
    run_frontend
    run_rust
    ;;
  frontend)
    run_frontend
    ;;
  rust)
    run_rust
    ;;
  *)
    echo "Usage: $0 [all|frontend|rust]" >&2
    exit 2
    ;;
esac
