#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-check}"

missing=0

require_tool() {
  local tool="$1"
  local install_hint="$2"
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'missing required tool: %s\n' "$tool" >&2
    printf 'install: %s\n' "$install_hint" >&2
    missing=1
    return 1
  fi
}

run_rust_audits() {
  if require_tool cargo-deny 'cargo install cargo-deny --locked'; then
    (cd "$ROOT/src-tauri" && cargo deny check advisories licenses bans sources)
  fi

  if require_tool cargo-audit 'cargo install cargo-audit --locked'; then
    cargo audit --manifest-path "$ROOT/src-tauri/Cargo.toml"
  fi
}

generate_sbom() {
  mkdir -p "$ROOT/dist/sbom"

  if require_tool cargo-cyclonedx 'cargo install cargo-cyclonedx --locked'; then
    (cd "$ROOT/src-tauri" && cargo cyclonedx --format json --output-cdx "$ROOT/dist/sbom/cargo.cdx.json")
  fi

  if command -v npx >/dev/null 2>&1; then
    npx --yes @cyclonedx/cyclonedx-npm --output-file "$ROOT/dist/sbom/npm.cdx.json" --ignore-npm-errors
  else
    printf 'missing required tool: npx\n' >&2
    printf 'install: Node.js/npm from the pinned project toolchain\n' >&2
    missing=1
  fi
}

case "$MODE" in
  check)
    run_rust_audits
    ;;
  sbom)
    generate_sbom
    ;;
  all)
    run_rust_audits
    generate_sbom
    ;;
  *)
    printf 'usage: %s [check|sbom|all]\n' "$0" >&2
    exit 2
    ;;
esac

if (( missing )); then
  printf 'supply-chain audit incomplete because required tooling is missing\n' >&2
  exit 1
fi
