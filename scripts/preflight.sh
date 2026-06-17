#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'Usage: npm run preflight -- <hook|quick|full|release>\n' >&2
  printf '  hook    shell syntax and app tauri-mock import guard\n' >&2
  printf '  quick   hook + npm run check; npm test\n' >&2
  printf '  full    quick + Rust fmt, clippy, and tests\n' >&2
  printf '  release full + license audit and production build\n' >&2
}

tier="${1:-quick}"
repo_root="$(git rev-parse --show-toplevel)"
dry_run="${CULL_PREFLIGHT_DRY_RUN:-0}"

cd "$repo_root"
export CI="${CI:-true}"

run() {
  printf '+'
  printf ' %q' "$@"
  printf '\n'
  if [ "$dry_run" = "1" ]; then
    return 0
  fi
  "$@"
}

check_shell_syntax() {
  while IFS= read -r script; do
    run bash -n "$script"
  done < <(find scripts -maxdepth 1 -type f -name '*.sh' | sort)
}

check_no_app_tauri_mock_imports() {
  echo "+ rg -n tauri-mock src --glob '!src/lib/tauri-mock.ts' --glob '!**/*.test.ts'"
  if [ "$dry_run" = "1" ]; then
    return 0
  fi

  if rg -n "tauri-mock" src --glob '!src/lib/tauri-mock.ts' --glob '!**/*.test.ts'; then
    cat >&2 <<'EOF'
Cull preflight failed: app source must not import tauri-mock.

`src/lib/tauri-mock.ts` is for browser E2E tests only. The app API layer and
components must keep using the real Tauri backend.
EOF
    exit 1
  fi
}

run_hook_checks() {
  check_shell_syntax
  check_no_app_tauri_mock_imports
}

run_frontend() {
  run npm run check
  run npm test
}

run_rust() {
  (
    cd src-tauri
    run cargo fmt --all -- --check
    run cargo clippy --locked --all-targets -- -D warnings
    run cargo test --all-targets
  )
}

run_release() {
  run npm run audit:licenses
  run npm run build
}

case "$tier" in
  hook)
    run_hook_checks
    ;;
  quick)
    run_hook_checks
    run_frontend
    ;;
  full)
    run_hook_checks
    run_frontend
    run_rust
    ;;
  release)
    run_hook_checks
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
