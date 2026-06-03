#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$ROOT"

if ! command -v bd >/dev/null 2>&1; then
  echo "bd is required to install Cull hooks" >&2
  exit 1
fi

bd hooks install --chain

BEGIN_MARKER="# --- BEGIN CULL WORKFLOW HOOK v1 ---"
END_MARKER="# --- END CULL WORKFLOW HOOK v1 ---"

section_for() {
  local hook_name="$1"
  cat <<EOF
$BEGIN_MARKER
# This section is managed by scripts/install-hooks.sh.
_cull_root=\$(git rev-parse --show-toplevel 2>/dev/null || pwd)
_cull_runner="\$_cull_root/scripts/hook-runner.sh"
if [ -x "\$_cull_runner" ]; then
  "\$_cull_runner" "$hook_name" "\$@"
  _cull_exit=\$?
  if [ \$_cull_exit -ne 0 ]; then exit \$_cull_exit; fi
fi
$END_MARKER
EOF
}

install_cull_section() {
  local hook_name="$1"
  local hook_path=".git/hooks/$hook_name"
  local temp_path
  temp_path="$(mktemp "${TMPDIR:-/tmp}/cull-hook-${hook_name}.XXXXXX")"

  if [ -f "$hook_path" ]; then
    awk -v begin="$BEGIN_MARKER" -v end="$END_MARKER" '
      $0 == begin { skip = 1; next }
      $0 == end { skip = 0; next }
      skip != 1 { print }
    ' "$hook_path" > "$temp_path"
  else
    printf '#!/usr/bin/env sh\n' > "$temp_path"
  fi

  printf '\n' >> "$temp_path"
  section_for "$hook_name" >> "$temp_path"
  mv "$temp_path" "$hook_path"
  chmod +x "$hook_path"
}

install_cull_section pre-commit
install_cull_section pre-push

echo "Cull hook chain installed:"
echo "  pre-commit -> scripts/preflight.sh hook"
echo "  pre-push   -> scripts/preflight.sh full"
