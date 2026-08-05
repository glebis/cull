#!/usr/bin/env bash
set -euo pipefail

export CI=true

target="${1:-all}"

run_frontend() {
  npm ci
  npm run lint:issues
  npm run check
  npm test
  npm run build
}

run_site() {
  (
    cd site
    npm ci
    npm run check
    npm test -- --run
    npm run build
  )
}

run_rust() {
  # tauri.conf.json bundles ../node_modules/@anthropic-ai/claude-agent-sdk as a
  # resource, so cargo needs node_modules present even for fmt/clippy/test.
  npm ci

  (
    cd src-tauri
    cargo fmt --all -- --check
    cargo clippy --locked --all-targets
    cargo test --locked --all-targets
  )
}

case "$target" in
  all)
    run_frontend
    run_rust
    run_site
    ;;
  frontend)
    run_frontend
    ;;
  rust)
    run_rust
    ;;
  site)
    run_site
    ;;
  *)
    echo "Usage: $0 [all|frontend|rust|site]" >&2
    exit 2
    ;;
esac
