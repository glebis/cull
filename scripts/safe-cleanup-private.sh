#!/usr/bin/env bash

# Safely retires a private directory created by the current invocation.
# Usage: safe_cleanup_private PATH EXPECTED_PARENT REQUIRED_BASENAME_PREFIX
#
# The path is validated as an owned, real directory directly beneath the
# expected parent. If the user's `trash` command exists, it is preferred. On
# minimal CI runners, the directory is atomically moved (never copied or
# deleted) into a private quarantine below RUNNER_TEMP/TMPDIR for runner-level
# cleanup. Cross-filesystem fallback fails closed rather than deleting files.
safe_cleanup_private() {
  local candidate=${1:-}
  local expected_parent=${2:-}
  local required_prefix=${3:-}
  [[ -n "$candidate" && -n "$expected_parent" && -n "$required_prefix" ]] || return 2
  [[ -e "$candidate" || -L "$candidate" ]] || return 0

  local validated
  validated="$(node - "$candidate" "$expected_parent" "$required_prefix" <<'NODE'
const fs = require('node:fs');
const path = require('node:path');
const [candidateInput, parentInput, prefix] = process.argv.slice(2);
const parent = fs.realpathSync(parentInput);
const candidateParent = fs.realpathSync(path.dirname(candidateInput));
if (candidateParent !== parent) process.exit(1);
const name = path.basename(candidateInput);
if (!name.startsWith(prefix) || name === prefix) process.exit(1);
const stat = fs.lstatSync(candidateInput);
if (!stat.isDirectory() || stat.isSymbolicLink()) process.exit(1);
if (typeof process.getuid === 'function' && stat.uid !== process.getuid()) process.exit(1);
const candidate = path.join(parent, name);
if (candidate === parent || parent.startsWith(candidate + path.sep)) process.exit(1);
process.stdout.write(candidate);
NODE
)" || return 1

  if command -v trash >/dev/null 2>&1; then
    trash "$validated"
    return
  fi

  local quarantine_base=${RUNNER_TEMP:-${TMPDIR:-/tmp}}
  node - "$validated" "$quarantine_base" <<'NODE'
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const [candidateInput, baseInput] = process.argv.slice(2);
const base = fs.realpathSync(baseInput);
const candidateParent = fs.realpathSync(path.dirname(candidateInput));
const candidate = path.join(candidateParent, path.basename(candidateInput));
const source = fs.lstatSync(candidate, { bigint: true });
if (!source.isDirectory() || source.isSymbolicLink()) process.exit(1);
const quarantine = path.join(base, '.cull-cleanup-quarantine');
try {
  fs.mkdirSync(quarantine, { mode: 0o700 });
} catch (error) {
  if (error.code !== 'EEXIST') process.exit(1);
}
const quarantineStat = fs.lstatSync(quarantine, { bigint: true });
if (!quarantineStat.isDirectory() || quarantineStat.isSymbolicLink()) process.exit(1);
if (typeof process.getuid === 'function' && Number(quarantineStat.uid) !== process.getuid()) process.exit(1);
if ((quarantineStat.mode & 0o077n) !== 0n || quarantineStat.dev !== source.dev) process.exit(1);
if (quarantine === candidate || quarantine.startsWith(candidate + path.sep)) process.exit(1);
const container = path.join(quarantine, `cull-${crypto.randomUUID()}`);
fs.mkdirSync(container, { mode: 0o700 });
fs.renameSync(candidate, path.join(container, 'owned-private-directory'));
NODE
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  safe_cleanup_private "$@"
fi
