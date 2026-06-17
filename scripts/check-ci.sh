#!/usr/bin/env bash
set -euo pipefail

export CI=true

target="${1:-all}"

run_frontend() {
  npm ci
  (cd site && npm ci)
  npm run lint:issues
  npm run check
  npm test
  npm run build
}

run_rust() {
  (
    cd src-tauri
    cargo fmt --all -- --check
    cargo clippy --locked --all-targets -- -D warnings
    cargo test --locked --all-targets
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
