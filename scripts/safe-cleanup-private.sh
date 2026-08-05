#!/usr/bin/env bash

# Safely retires a private directory created by the current invocation.
# Usage: safe_cleanup_private PATH EXPECTED_PARENT REQUIRED_BASENAME_PREFIX
#
# The helper first atomically claims the exact validated inode by renaming it
# into a new mode-0700 sibling container on the same filesystem. It never acts
# on the original basename after that claim. If `trash` exists, only the unique
# claimed container is sent there. Minimal runners retain that uniquely named
# sibling container for runner/worktree cleanup; no recursive deletion, fixed
# quarantine path, symlink following, or cross-filesystem move is used.
safe_cleanup_private() {
  local candidate=${1:-}
  local expected_parent=${2:-}
  local required_prefix=${3:-}
  [[ -n "$candidate" && -n "$expected_parent" && -n "$required_prefix" ]] || return 2
  [[ -e "$candidate" || -L "$candidate" ]] || return 0

  local claim_result
  claim_result="$(node - "$candidate" "$expected_parent" "$required_prefix" <<'NODE'
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const [candidateInput, parentInput, prefix] = process.argv.slice(2);
const parent = fs.realpathSync(parentInput);
const candidateParent = fs.realpathSync(path.dirname(candidateInput));
if (candidateParent !== parent) process.exit(1);
const name = path.basename(candidateInput);
if (!name.startsWith(prefix) || name === prefix) process.exit(1);
const candidate = path.join(parent, name);
const initial = fs.lstatSync(candidate, { bigint: true });
if (!initial.isDirectory() || initial.isSymbolicLink()) process.exit(1);
if (typeof process.getuid === 'function' && Number(initial.uid) !== process.getuid()) process.exit(1);
const identity = stat => `${stat.dev}:${stat.ino}`;
const expectedIdentity = identity(initial);
const container = path.join(parent, `.cull-cleanup-claim.${crypto.randomUUID()}`);
fs.mkdirSync(container, { mode: 0o700 });
const containerInitial = fs.lstatSync(container, { bigint: true });
if (!containerInitial.isDirectory() || containerInitial.isSymbolicLink() ||
    (containerInitial.mode & 0o077n) !== 0n || containerInitial.dev !== initial.dev) process.exit(1);

const testMode = process.env.CULL_RELEASE_TEST_MODE === '1';
if (testMode && process.env.CULL_SAFE_CLEANUP_TEST_RACE_CONTAINER === '1') {
  fs.renameSync(container, `${container}.preserved-original`);
  fs.mkdirSync(container, { mode: 0o700 });
}
const containerCurrent = fs.lstatSync(container, { bigint: true });
if (identity(containerCurrent) !== identity(containerInitial) || !containerCurrent.isDirectory() || containerCurrent.isSymbolicLink()) process.exit(1);

const swapWith = testMode ? process.env.CULL_SAFE_CLEANUP_TEST_SWAP_WITH : '';
if (swapWith) {
  fs.renameSync(candidate, path.join(container, 'preserved-original'));
  fs.renameSync(swapWith, candidate);
}
const current = fs.lstatSync(candidate, { bigint: true });
if (!current.isDirectory() || current.isSymbolicLink() || identity(current) !== expectedIdentity) process.exit(1);

const claimed = path.join(container, 'claimed-private-directory');
fs.renameSync(candidate, claimed);
const claimedStat = fs.lstatSync(claimed, { bigint: true });
if (!claimedStat.isDirectory() || claimedStat.isSymbolicLink() || identity(claimedStat) !== expectedIdentity) process.exit(1);
process.stdout.write(`${container}\t${identity(containerInitial)}`);
NODE
)" || return 1

  local claimed_container=${claim_result%%$'\t'*}
  local claimed_identity=${claim_result#*$'\t'}
  [[ -n "$claimed_container" && "$claimed_identity" != "$claim_result" ]] || return 1

  node - "$claimed_container" "$claimed_identity" <<'NODE' || return 1
const fs = require('node:fs');
const [container, expected] = process.argv.slice(2);
const stat = fs.lstatSync(container, { bigint: true });
if (!stat.isDirectory() || stat.isSymbolicLink() || `${stat.dev}:${stat.ino}` !== expected) process.exit(1);
NODE

  if command -v trash >/dev/null 2>&1; then
    trash "$claimed_container"
  fi
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  safe_cleanup_private "$@"
fi
