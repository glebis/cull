#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: npm run preflight -- <quick|full|release>\n' >&2
  printf '  quick   npm run check; npm test\n' >&2
  printf '  full    quick + Rust fmt, clippy, and tests\n' >&2
  printf '  release full + license audit and production build\n' >&2
}

tier="${1:-quick}"
repo_root="$(git rev-parse --show-toplevel)"

cd "$repo_root"
export CI="${CI:-true}"

run_frontend() {
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

run_release() {
  npm run audit:licenses
  npm run build
}

case "$tier" in
  quick)
    run_frontend
    ;;
  full)
    run_frontend
    run_rust
    ;;
  release)
    run_frontend
    run_rust
    run_release
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    usage
    exit 2
    ;;
esac
