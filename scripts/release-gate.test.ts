import { execFileSync, spawnSync } from 'node:child_process';
import {
  existsSync,
  linkSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  renameSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const gate = resolve(import.meta.dirname, 'release-gate.mjs');
const canaryWorkflowPath = resolve(import.meta.dirname, '../.github/workflows/release-canary.yml');
const releaseWorkflowPath = resolve(import.meta.dirname, '../.github/workflows/release.yml');
const tapWorkflowPath = resolve(import.meta.dirname, '../.github/workflows/update-tap.yml');
const DB_CONTRACT = 'cargo test --manifest-path src-tauri/Cargo.toml --features test-support --test compat_golden';
const EXPORT_CONTRACT = 'cargo test --manifest-path src-tauri/Cargo.toml --features test-support --test export_compat_golden';

function workflowJob(workflow: string, name: string) {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = new RegExp(`^  ${escaped}:\\n([\\s\\S]*?)(?=^  [a-zA-Z0-9_-]+:\\n|(?![\\s\\S]))`, 'm').exec(workflow);
  expect(match, `missing workflow job ${name}`).not.toBeNull();
  return match![1];
}

function write(root: string, path: string, contents: string) {
  const destination = join(root, path);
  mkdirSync(dirname(destination), { recursive: true });
  writeFileSync(destination, contents);
}

function git(root: string, ...args: string[]) {
  return execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
}

function commit(root: string, message: string) {
  git(root, 'add', '.');
  git(root, '-c', 'user.name=Cull Test', '-c', 'user.email=cull@example.test', 'commit', '-m', message);
  return git(root, 'rev-parse', 'HEAD');
}

function annotatedTag(root: string, name: string, sha: string, force = false) {
  git(root, 'tag', ...(force ? ['-f'] : []), '-a', name, sha, '-m', `Release ${name}`);
}

function metadata(root: string, version: string) {
  write(root, 'package.json', JSON.stringify({ name: 'cull', version }));
  write(root, 'package-lock.json', JSON.stringify({ version, packages: { '': { version } } }));
  write(root, 'src-tauri/tauri.conf.json', JSON.stringify({ version }));
  write(root, 'src-tauri/Cargo.toml', `[package]\nname = "cull"\nversion = "${version}"\n`);
  write(root, 'src-tauri/Cargo.lock', `version = 4\n\n[[package]]\nname = "cull"\nversion = "${version}"\n`);
}

function config(root: string, extraGate = [DB_CONTRACT, EXPORT_CONTRACT]) {
  write(root, 'release.config.json', JSON.stringify({
    schemaVersion: 1,
    minimumFreeDiskGiB: 1,
    versionFiles: [
      { id: 'package', path: 'package.json', kind: 'json', pointers: ['/version'] },
      { id: 'package-lock', path: 'package-lock.json', kind: 'json', pointers: ['/version', '/packages//version'] },
      { id: 'tauri', path: 'src-tauri/tauri.conf.json', kind: 'json', pointers: ['/version'] },
      { id: 'cargo', path: 'src-tauri/Cargo.toml', kind: 'toml-package-version', package: 'cull' },
      { id: 'cargo-lock', path: 'src-tauri/Cargo.lock', kind: 'cargo-lock-package-version', package: 'cull' },
    ],
    extraGate,
    changelog: { path: 'CHANGELOG.md' },
    compatibility: { path: 'docs/COMPATIBILITY.md' },
    e2e: { exact: ['src/lib/api.ts'], prefixes: ['src/lib/components/', 'tests/e2e/'] },
  }, null, 2));
}

function fixture() {
  const root = mkdtempSync(join(tmpdir(), 'cull-release-gate-'));
  git(root, 'init', '-b', 'main');
  config(root);
  metadata(root, '1.2.3');
  write(root, 'CHANGELOG.md', '# Changelog\n\n## [1.2.3] - 2026-07-01\n');
  write(root, 'docs/COMPATIBILITY.md', 'Last updated: 1.2.3 (2026-07-01)\n');
  const stableApiBody = Array.from({ length: 20 }, (_, index) => `export const stable${index} = ${index};`).join('\n');
  write(root, 'src/lib/api.ts', `export const version = 1;\n${stableApiBody}\n`);
  const baseSha = commit(root, 'base');
  annotatedTag(root, 'v1.2.3', baseSha);

  metadata(root, '1.2.4');
  write(root, 'CHANGELOG.md', '# Changelog\n\n## [1.2.4] - 2026-07-11\n\n### Fixed\n\n- Safe release gates.\n\n## [1.2.3] - 2026-07-01\n');
  write(root, 'docs/COMPATIBILITY.md', 'Last updated: 1.2.4 (2026-07-11)\n');
  write(root, 'src/lib/api.ts', `export const version = 2;\n${stableApiBody}\n`);
  const sha = commit(root, 'release');
  annotatedTag(root, 'v1.2.4', sha);
  git(root, 'update-ref', 'refs/remotes/origin/main', sha);
  return { root, baseSha, sha };
}

function untaggedMainCanaryFixture() {
  const { root, baseSha, sha: taggedSha } = fixture();
  write(root, 'src/lib/api.ts', `${readFileSync(join(root, 'src/lib/api.ts'), 'utf8')}\nexport const canary = true;\n`);
  const sha = commit(root, 'untagged main after release');
  git(root, 'update-ref', 'refs/remotes/origin/main', sha);
  return { root, baseSha, taggedSha, sha };
}

function run(root: string, options: Partial<{
  tag: string;
  sha: string;
  baseTag: string;
  event: string;
  jsonOut: string;
  env: NodeJS.ProcessEnv;
}> = {}) {
  const sha = options.sha ?? git(root, 'rev-parse', 'v1.2.4^{commit}');
  const jsonOut = options.jsonOut ?? join(root, 'gate-output.json');
  const execution = spawnSync(process.execPath, [
    gate,
    '--tag', options.tag ?? 'v1.2.4',
    '--sha', sha,
    '--base-tag', options.baseTag ?? 'v1.2.3',
    '--event', options.event ?? 'tag',
    '--json-out', jsonOut,
  ], { cwd: root, encoding: 'utf8', env: { ...process.env, ...options.env } });
  return { execution, jsonOut };
}

function expectRejected(result: ReturnType<typeof run>, code: string) {
  expect(result.execution.status).toBe(2);
  expect(JSON.parse(result.execution.stderr)).toMatchObject({ ok: false, code });
  expect(result.execution.stdout).toBe('');
}

describe('release gate', () => {
  it('emits the exact commit-bound gate record and records conditional E2E', () => {
    const { root, sha } = fixture();
    const result = run(root);

    expect(result.execution.status).toBe(0);
    const output = JSON.parse(result.execution.stdout);
    const tagObjectSha = git(root, 'rev-parse', 'refs/tags/v1.2.4');
    expect(output).toEqual({
      schema: 'cull.release.gate.v1',
      event: 'tag',
      publishEligible: true,
      version: '1.2.4',
      tag: 'v1.2.4',
      sha,
      tagObjectSha,
      baseTag: 'v1.2.3',
      mainAncestor: true,
      versions: {
        package: ['1.2.4'],
        'package-lock': ['1.2.4', '1.2.4'],
        tauri: ['1.2.4'],
        cargo: ['1.2.4'],
        'cargo-lock': ['1.2.4'],
      },
      e2e: { required: true, matchedPaths: ['src/lib/api.ts'] },
      commands: [
        'npm run audit:licenses',
        'bash scripts/supply-chain-audit.sh check',
        DB_CONTRACT,
        EXPORT_CONTRACT,
        'bash tests/e2e/run-e2e.sh',
        'npm run build',
      ],
    });
    expect(JSON.parse(readFileSync(result.jsonOut, 'utf8'))).toEqual(output);
  });

  it('rejects malformed tags', () => {
    const { root } = fixture();
    expectRejected(run(root, { tag: '1.2.4' }), 'INPUT_INVALID');
  });

  it('rejects multiline output paths before they can become workflow outputs', () => {
    const { root } = fixture();
    expectRejected(run(root, { jsonOut: `${join(root, 'gate.json')}\nbad=value` }), 'INPUT_INVALID');
  });

  it('rejects a tag that does not resolve to the supplied SHA', () => {
    const { root, baseSha } = fixture();
    expectRejected(run(root, { sha: baseSha }), 'TAG_SHA_MISMATCH');
  });

  it('rejects a lightweight release tag even when it peels to the supplied SHA', () => {
    const { root, sha } = fixture();
    git(root, 'tag', '-d', 'v1.2.4');
    git(root, 'tag', 'v1.2.4', sha);

    expectRejected(run(root, { sha }), 'TAG_NOT_ANNOTATED');
  });

  it('rejects a release SHA that is not reachable from origin/main', () => {
    const { root, baseSha } = fixture();
    git(root, 'switch', '--detach', baseSha);
    metadata(root, '1.2.4');
    write(root, 'CHANGELOG.md', '# Changelog\n\n## [1.2.4] - 2026-07-11\n');
    write(root, 'docs/COMPATIBILITY.md', 'Last updated: 1.2.4 (2026-07-11)\n');
    const divergent = commit(root, 'divergent release');
    git(root, 'tag', 'v1.2.4-divergent', divergent);
    annotatedTag(root, 'v1.2.4', divergent, true);

    expectRejected(run(root, { sha: divergent }), 'NOT_ON_ORIGIN_MAIN');
  });

  it('classifies the covered source of a rename even when Git rename detection is enabled', () => {
    const { root } = fixture();
    mkdirSync(join(root, 'docs'), { recursive: true });
    renameSync(join(root, 'src/lib/api.ts'), join(root, 'docs/renamed-api.ts'));
    const sha = commit(root, 'rename covered source outside E2E policy');
    annotatedTag(root, 'v1.2.4', sha, true);
    git(root, 'update-ref', 'refs/remotes/origin/main', sha);
    git(root, 'config', 'diff.renames', 'copies');

    const result = run(root, { sha });
    expect(result.execution.status).toBe(0);
    expect(JSON.parse(result.execution.stdout).e2e).toEqual({
      required: true,
      matchedPaths: ['src/lib/api.ts'],
    });
  });

  it('rejects a reachable injected lower tag that would narrow the release diff', () => {
    const { root, sha } = fixture();
    git(root, 'tag', 'v1.2.2', sha);

    expectRejected(run(root, { baseTag: 'v1.2.2' }), 'BASE_TAG_MISMATCH');
  });

  it('rejects mismatched version metadata at the release commit', () => {
    const { root } = fixture();
    const released = git(root, 'rev-parse', 'HEAD');
    write(root, 'package.json', JSON.stringify({ name: 'cull', version: '9.9.9' }));
    const mismatched = commit(root, 'mismatched metadata');
    annotatedTag(root, 'v1.2.4', mismatched, true);
    git(root, 'update-ref', 'refs/remotes/origin/main', mismatched);
    expect(released).not.toBe(mismatched);

    expectRejected(run(root, { sha: mismatched }), 'VERSION_MISMATCH');
  });

  it('rejects a missing changelog stamp for the version', () => {
    const { root } = fixture();
    write(root, 'CHANGELOG.md', '# Changelog\n\n## [1.2.3] - 2026-07-01\n');
    const sha = commit(root, 'missing changelog stamp');
    annotatedTag(root, 'v1.2.4', sha, true);
    git(root, 'update-ref', 'refs/remotes/origin/main', sha);

    expectRejected(run(root, { sha }), 'CHANGELOG_INVALID');
  });

  it('rejects release configuration missing a stable contract command', () => {
    const { root } = fixture();
    config(root, [DB_CONTRACT]);
    const sha = commit(root, 'missing stable export gate');
    annotatedTag(root, 'v1.2.4', sha, true);
    git(root, 'update-ref', 'refs/remotes/origin/main', sha);

    expectRejected(run(root, { sha }), 'STABLE_CONTRACT_MISSING');
  });

  it('rejects E2E evidence that omits a classified changed path', async () => {
    const { assertE2ERecorded } = await import('./release-gate.mjs');
    expect(() => assertE2ERecorded(['src/lib/api.ts'], { required: false, matchedPaths: [] }))
      .toThrowError(expect.objectContaining({ code: 'E2E_EVIDENCE_INVALID' }));
  });

  it('requires existing tag identity for manual dispatch', () => {
    const { root } = fixture();
    expectRejected(run(root, { tag: 'v1.2.5', event: 'dispatch' }), 'TAG_NOT_FOUND');
  });

  it('accepts untagged current main only as explicit non-publishing canary evidence', () => {
    const { root, taggedSha, sha } = untaggedMainCanaryFixture();
    const workflowOutput = join(root, 'github-output');
    const result = run(root, {
      sha,
      baseTag: 'v1.2.4',
      event: 'canary',
      env: { GITHUB_OUTPUT: workflowOutput },
    });

    expect(taggedSha).not.toBe(sha);
    expect(result.execution.status).toBe(0);
    expect(JSON.parse(result.execution.stdout)).toMatchObject({
      schema: 'cull.release.gate.v1',
      event: 'canary',
      publishEligible: false,
      version: '1.2.4',
      tag: 'v1.2.4',
      sha,
      tagObjectSha: null,
      baseTag: 'v1.2.4',
      mainAncestor: true,
      e2e: { required: true, matchedPaths: ['src/lib/api.ts'] },
    });
    expect(readFileSync(workflowOutput, 'utf8')).toContain('event=canary\npublish_eligible=false\n');
    expect(readFileSync(workflowOutput, 'utf8')).toContain('tag_object_sha=\n');
  });

  it('keeps dispatch tag binding strict for the same untagged main SHA', () => {
    const { root, sha } = untaggedMainCanaryFixture();

    expectRejected(run(root, { sha, baseTag: 'v1.2.3', event: 'dispatch' }), 'TAG_SHA_MISMATCH');
  });

  it('rejects an injected lower canary base that would narrow the canonical diff', () => {
    const { root, sha } = untaggedMainCanaryFixture();
    git(root, 'tag', '-f', 'v1.2.3', sha);

    expectRejected(run(root, { sha, baseTag: 'v1.2.3', event: 'canary' }), 'BASE_TAG_MISMATCH');
  });

  it('marks bound manual dispatch evidence publish eligible only after tag identity succeeds', () => {
    const { root } = fixture();
    const result = run(root, { event: 'dispatch' });

    expect(result.execution.status).toBe(0);
    expect(JSON.parse(result.execution.stdout)).toMatchObject({
      event: 'dispatch',
      publishEligible: true,
      tag: 'v1.2.4',
    });
  });

  it('rejects the JSON artifact and workflow output as the same normalized destination', () => {
    const { root } = fixture();
    const output = join(root, 'nested', '..', 'gate.json');
    const normalized = join(root, 'gate.json');

    expectRejected(run(root, { jsonOut: output, env: { GITHUB_OUTPUT: normalized } }), 'OUTPUT_ALIAS');
    expect(existsSync(normalized)).toBe(false);
  });

  it.each([
    ['symbolic link', (alias: string, target: string) => symlinkSync(target, alias)],
    ['hard link', (alias: string, target: string) => linkSync(target, alias)],
  ])('rejects a %s alias of the workflow output before writing', (_name, makeAlias) => {
    const { root } = fixture();
    const workflowOutput = join(root, 'github-output');
    const jsonOut = join(root, 'gate-alias.json');
    writeFileSync(workflowOutput, 'sentinel\n');
    makeAlias(jsonOut, workflowOutput);

    expectRejected(run(root, { jsonOut, env: { GITHUB_OUTPUT: workflowOutput } }), 'OUTPUT_ALIAS');
    expect(readFileSync(workflowOutput, 'utf8')).toBe('sentinel\n');
  });

  it('does not create a JSON artifact when appending workflow outputs fails', () => {
    const { root } = fixture();
    const jsonOut = join(root, 'gate.json');

    expectRejected(run(root, { jsonOut, env: { GITHUB_OUTPUT: root } }), 'WORKFLOW_OUTPUT_FAILED');
    expect(existsSync(jsonOut)).toBe(false);
  });

  it('preserves a preexisting JSON destination when appending workflow outputs fails', () => {
    const { root } = fixture();
    const jsonOut = join(root, 'gate.json');
    writeFileSync(jsonOut, 'preexisting artifact\n');

    expectRejected(run(root, { jsonOut, env: { GITHUB_OUTPUT: root } }), 'WORKFLOW_OUTPUT_FAILED');
    expect(readFileSync(jsonOut, 'utf8')).toBe('preexisting artifact\n');
  });

  it('keeps local release preflight deterministic and complete', () => {
    const preflight = readFileSync(resolve(import.meta.dirname, 'preflight.sh'), 'utf8');
    expect(preflight).not.toContain('CULL_PREFLIGHT_SKIP_E2E');
    const commands = [
      'run npm run audit:licenses',
      'run bash scripts/supply-chain-audit.sh check',
      `run ${DB_CONTRACT}`,
      `run ${EXPORT_CONTRACT}`,
      'run npm run build',
    ];
    let cursor = -1;
    for (const command of commands) {
      const next = preflight.indexOf(command);
      expect(next, `missing ${command}`).toBeGreaterThan(cursor);
      cursor = next;
    }
  });

  it('hardens CI and checks the site as an explicit job', () => {
    const workflow = readFileSync(resolve(import.meta.dirname, '../.github/workflows/ci.yml'), 'utf8');
    expect(workflow).toContain('permissions:\n  contents: read');
    expect(workflow).toContain('group: ci-${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}');
    expect(workflow).toContain('cancel-in-progress: true');
    expect(workflow.match(/uses: Swatinem\/rust-cache@/g)).toHaveLength(2);
    expect(workflow.match(/workspaces: src-tauri -> target/g)).toHaveLength(2);
    expect(workflow).toMatch(/\n  site:\n[\s\S]*?working-directory: site[\s\S]*?npm ci[\s\S]*?npm run check[\s\S]*?npm test[\s\S]*?npm run build/);

    const ciScript = readFileSync(resolve(import.meta.dirname, 'check-ci.sh'), 'utf8');
    expect(ciScript).toContain('run_site');
    expect(ciScript).toContain('all|frontend|rust|site');
  });

  it('configures weekly Dependabot updates for the site package', () => {
    const dependabot = readFileSync(resolve(import.meta.dirname, '../.github/dependabot.yml'), 'utf8');
    expect(dependabot).toMatch(/package-ecosystem: "npm"\n\s+directory: "\/site"/);
  });

  it('defines a read-only, serialized manual release canary', () => {
    const workflow = readFileSync(canaryWorkflowPath, 'utf8');

    expect(workflow).toMatch(/workflow_dispatch:\n\s+inputs:\n\s+ref:\n[\s\S]*?default: main/);
    expect(workflow).toContain('permissions:\n  contents: read');
    expect(workflow).toContain('group: release-canary');
    expect(workflow).toContain('cancel-in-progress: true');
    const jobs = workflow.split('\njobs:\n')[1];
    expect([...jobs.matchAll(/^  ([a-zA-Z0-9_-]+):$/gm)].map((match) => match[1]))
      .toEqual(['gate', 'signed-build', 'verify']);
    expect(workflowJob(workflow, 'signed-build')).toContain('needs: gate');
    expect(workflowJob(workflow, 'verify')).toContain('needs: [gate, signed-build]');
  });

  it('pins every canary action and contains no publishing capability', () => {
    const workflow = readFileSync(canaryWorkflowPath, 'utf8');
    const actionUses = [...workflow.matchAll(/uses:\s+([^\s#]+)/g)].map((match) => match[1]);

    expect(actionUses.length).toBeGreaterThan(0);
    expect(actionUses.every((action) => /@[0-9a-f]{40}$/.test(action))).toBe(true);
    for (const forbidden of [
      'contents: write', 'gh release', 'git tag', 'git push', 'tagName', 'releaseName',
      'releaseDraft', 'releaseId', 'HOMEBREW_TAP_TOKEN', 'GITHUB_TOKEN', 'GH_TOKEN',
    ]) {
      expect(workflow).not.toContain(forbidden);
    }
  });

  it('confines signing secrets to the signed build after gate evidence and conditional E2E', () => {
    const workflow = readFileSync(canaryWorkflowPath, 'utf8');
    const gateJob = workflowJob(workflow, 'gate');
    const buildJob = workflowJob(workflow, 'signed-build');
    const verifyJob = workflowJob(workflow, 'verify');

    expect(gateJob).not.toContain('${{ secrets.');
    expect(verifyJob).not.toContain('${{ secrets.');
    expect(buildJob).toContain('${{ secrets.APPLE_CERTIFICATE }}');
    expect(buildJob).toContain('${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}');
    expect(buildJob).toContain('tauri-apps/tauri-action@');
    expect(buildJob).toContain('--target aarch64-apple-darwin --bundles dmg');
    expect(buildJob).not.toContain('--bundles dmg,updater');
    expect(gateJob).toContain('node scripts/release-gate.mjs');
    expect(gateJob).toContain('--event canary');
    expect(gateJob).not.toContain('--event dispatch');
    expect(gateJob).toContain("if: steps.release_gate.outputs.e2e_required == 'true'");
    expect(gateJob).toContain('bash tests/e2e/run-e2e.sh');
    expect(workflow).toContain('event: ${{ steps.release_gate.outputs.event }}');
    expect(workflow).toContain('publish_eligible: ${{ steps.release_gate.outputs.publish_eligible }}');
    expect(buildJob).not.toContain("if: needs.gate.outputs.event == 'canary' && needs.gate.outputs.publish_eligible == 'false'");
    expect(buildJob).toContain('GATE_EVENT: ${{ needs.gate.outputs.event }}');
    expect(buildJob).toContain('PUBLISH_ELIGIBLE: ${{ needs.gate.outputs.publish_eligible }}');
    expect(buildJob).toContain('test "$GATE_EVENT" = canary');
    expect(buildJob).toContain('test "$PUBLISH_ELIGIBLE" = false');
    const evidenceCheck = buildJob.indexOf('Assert non-publishing canary evidence');
    const firstSecret = buildJob.indexOf('${{ secrets.');
    expect(evidenceCheck).toBeGreaterThan(-1);
    expect(firstSecret).toBeGreaterThan(evidenceCheck);
  });

  it('passes the private signed inventory to a secret-free exact verifier', () => {
    const workflow = readFileSync(canaryWorkflowPath, 'utf8');
    const buildJob = workflowJob(workflow, 'signed-build');
    const verifyJob = workflowJob(workflow, 'verify');

    expect(buildJob).toContain('name: cull-canary-${{ github.run_id }}');
    expect(buildJob).toContain('retention-days: 1');
    expect(verifyJob).toContain('name: cull-canary-${{ github.run_id }}');
    expect(verifyJob).toContain('bash scripts/verify-release-artifacts.sh');
    expect(verifyJob).toContain('--artifact-dir "$RUNNER_TEMP/cull-canary"');
    expect(verifyJob).toContain('--out "$RUNNER_TEMP/cull-canary-evidence"');
    expect(verifyJob).toContain('minisign-0.12-macos.zip');
    expect(verifyJob).toContain('89000b19535765f9cffc65a65d64a820f433ef6db8020667f7570e06bf6aac63');
    expect(verifyJob).toContain('release-provenance.json');
    expect(verifyJob).toContain('checksums.txt');
    expect(verifyJob).toContain('verification.log');
    expect(verifyJob).not.toContain('--tag-object-sha');
  });

  it('defines a pinned four-stage release workflow with immutable tag dispatch', () => {
    const workflow = readFileSync(releaseWorkflowPath, 'utf8');
    const jobs = workflow.split('\njobs:\n')[1];

    expect(workflow).toMatch(/workflow_dispatch:\n\s+inputs:\n\s+tag:\n[\s\S]*?required: true[\s\S]*?type: string/);
    expect(workflow).toContain("tags:\n      - 'v*'");
    expect(workflow).toContain('group: release-${{ inputs.tag || github.ref_name }}');
    expect(workflow).toContain('cancel-in-progress: false');
    expect([...jobs.matchAll(/^  ([a-zA-Z0-9_-]+):$/gm)].map((match) => match[1]))
      .toEqual(['release-gate', 'signed-build', 'verify-artifact', 'publish']);
    expect(workflowJob(workflow, 'signed-build')).toContain('needs: release-gate');
    expect(workflowJob(workflow, 'verify-artifact')).toContain('needs: [release-gate, signed-build]');
    expect(workflowJob(workflow, 'publish')).toContain('needs: [release-gate, signed-build, verify-artifact]');

    const actionUses = [...workflow.matchAll(/uses:\s+([^\s#]+)/g)].map((match) => match[1]);
    expect(actionUses.length).toBeGreaterThan(0);
    expect(actionUses.every((action) => /@[0-9a-f]{40}$/.test(action))).toBe(true);
  });

  it('keeps checks and verification secret-free and grants write only to publish', () => {
    const workflow = readFileSync(releaseWorkflowPath, 'utf8');
    const gateJob = workflowJob(workflow, 'release-gate');
    const buildJob = workflowJob(workflow, 'signed-build');
    const verifyJob = workflowJob(workflow, 'verify-artifact');
    const publishJob = workflowJob(workflow, 'publish');

    expect(workflow).toContain('permissions:\n  contents: read');
    expect(gateJob).not.toContain('${{ secrets.');
    expect(verifyJob).not.toContain('${{ secrets.');
    expect(gateJob).not.toContain('contents: write');
    expect(buildJob).not.toContain('contents: write');
    expect(verifyJob).not.toContain('contents: write');
    expect(publishJob).toContain('contents: write');
    expect(buildJob).toContain('${{ secrets.APPLE_CERTIFICATE }}');
    expect(buildJob).toContain('${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}');
    expect(buildJob).not.toContain('HOMEBREW_TAP_TOKEN');
    expect(publishJob).not.toContain('${{ secrets.');
    expect(publishJob).not.toContain('APPLE_CERTIFICATE');
    expect(publishJob).not.toContain('TAURI_SIGNING_PRIVATE_KEY');
  });

  it('finishes every gate including conditional E2E before exposing signing secrets', () => {
    const workflow = readFileSync(releaseWorkflowPath, 'utf8');
    const gateJob = workflowJob(workflow, 'release-gate');
    const buildJob = workflowJob(workflow, 'signed-build');

    expect(gateJob).toContain('fetch-depth: 0');
    expect(gateJob).toContain('persist-credentials: false');
    expect(gateJob).toContain('git cat-file -t "refs/tags/$tag"');
    expect(gateJob).toContain('node scripts/release-gate.mjs');
    expect(gateJob).toContain('--event "$event"');
    expect(gateJob).toContain('tag-object-sha: ${{ steps.release_gate.outputs.tag_object_sha }}');
    for (const command of [
      'npm run audit:licenses',
      'bash scripts/supply-chain-audit.sh check',
      DB_CONTRACT,
      EXPORT_CONTRACT,
      'bash tests/e2e/run-e2e.sh',
      'npm run build',
    ]) expect(gateJob).toContain(command);
    expect(gateJob).toContain("if: steps.release_gate.outputs.e2e_required == 'true'");
    expect(buildJob).toContain('GATE_EVENT: ${{ needs.release-gate.outputs.event }}');
    expect(buildJob).toContain('PUBLISH_ELIGIBLE: ${{ needs.release-gate.outputs.publish-eligible }}');
    expect(buildJob).toContain('test "$PUBLISH_ELIGIBLE" = true');
    expect(buildJob.indexOf('Assert publish-eligible gate evidence'))
      .toBeLessThan(buildJob.indexOf('${{ secrets.'));
  });

  it('binds exact signed and verified artifacts to this run without publishing from build', () => {
    const workflow = readFileSync(releaseWorkflowPath, 'utf8');
    const buildJob = workflowJob(workflow, 'signed-build');
    const verifyJob = workflowJob(workflow, 'verify-artifact');

    expect(buildJob).toContain('tauri-apps/tauri-action@');
    expect(buildJob).toContain('--target aarch64-apple-darwin --bundles dmg');
    for (const forbidden of ['tagName:', 'releaseName:', 'releaseDraft:', 'gh release']) {
      expect(buildJob).not.toContain(forbidden);
    }
    expect(buildJob).toContain('name: cull-signed-${{ github.run_id }}-${{ needs.release-gate.outputs.sha }}');
    expect(buildJob).toContain('artifact-id: ${{ steps.upload_signed.outputs.artifact-id }}');
    expect(buildJob).toContain('artifact-digest: ${{ steps.upload_signed.outputs.artifact-digest }}');
    expect(verifyJob).toContain('artifact-ids: ${{ needs.signed-build.outputs.artifact-id }}');
    expect(verifyJob).toContain('merge-multiple: true');
    expect(verifyJob).toContain('gh api "repos/$REPOSITORY/actions/artifacts/$ARTIFACT_ID"');
    expect(verifyJob).toContain('artifact.workflow_run?.id');
    expect(verifyJob).toContain('artifact.workflow_run?.head_sha');
    expect(verifyJob).toContain('EXPECTED_INVOCATION_SHA: ${{ github.sha }}');
    expect(verifyJob).not.toContain('EXPECTED_SHA: ${{ needs.release-gate.outputs.sha }}');
    expect(verifyJob).toContain('artifact.expired !== false');
    expect(verifyJob).toContain('artifact.digest !== expectedDigest');
    expect(verifyJob).toContain('bash scripts/verify-release-artifacts.sh');
    expect(verifyJob).toContain('--run-id "${{ github.run_id }}"');
    expect(verifyJob).toContain('--tag-object-sha "${{ needs.release-gate.outputs.tag-object-sha }}"');
    expect(verifyJob).toContain('name: cull-verified-${{ github.run_id }}-${{ needs.release-gate.outputs.sha }}');
    expect(verifyJob).toContain('artifact-id: ${{ steps.upload_verified.outputs.artifact-id }}');
    expect(verifyJob).toContain('artifact-digest: ${{ steps.upload_verified.outputs.artifact-digest }}');
    expect(verifyJob).not.toContain('gh release');
  });

  it('keeps automatic publication disabled and publishes only exact verified files', () => {
    const workflow = readFileSync(releaseWorkflowPath, 'utf8');
    const publishJob = workflowJob(workflow, 'publish');
    const releaseDocs = readFileSync(resolve(import.meta.dirname, '../docs/RELEASING.md'), 'utf8');

    expect(publishJob).toContain("vars.CULL_RELEASE_PUBLISH_ENABLED == 'true'");
    expect(publishJob).toContain("needs.release-gate.outputs.publish-eligible == 'true'");
    expect(publishJob).toContain("needs.release-gate.outputs.event == 'tag'");
    expect(publishJob).toContain("needs.release-gate.outputs.event == 'dispatch'");
    expect(publishJob).toContain('environment: release-publish');
    expect(publishJob).toContain('artifact-ids: ${{ needs.verify-artifact.outputs.artifact-id }}');
    expect(publishJob).toContain('merge-multiple: true');
    expect(publishJob).toContain('gh api "repos/$REPOSITORY/actions/artifacts/$ARTIFACT_ID"');
    expect(publishJob).toContain('artifact.workflow_run?.id');
    expect(publishJob).toContain('artifact.workflow_run?.head_sha');
    expect(publishJob).toContain('EXPECTED_INVOCATION_SHA: ${{ github.sha }}');
    expect(publishJob).not.toContain('EXPECTED_SHA: ${{ needs.release-gate.outputs.sha }}');
    expect(publishJob).toContain('artifact.expired !== false');
    expect(publishJob).toContain('artifact.digest !== expectedDigest');
    expect(publishJob).toContain("schema !== 'cull.release.gate.v1'");
    expect(publishJob).toContain('gate.publishEligible !== true');
    expect(publishJob).toContain("gate.event !== 'tag' && gate.event !== 'dispatch'");
    expect(publishJob).toContain('gate.tagObjectSha !== process.env.TAG_OBJECT_SHA');
    expect(publishJob).toContain("schema !== 'cull.release.provenance.v1'");
    expect(publishJob).toContain('provenance.tagObjectSha !== process.env.TAG_OBJECT_SHA');
    expect(publishJob).toContain("'stapledNotarization'");
    expect(publishJob).toContain('Object.keys(provenance.checks).sort()');
    expect(publishJob).toContain('provenance.checks[name] !== true');
    expect(publishJob).toContain("heading === 'Unreleased'");
    expect(publishJob).toContain('gh release create "$TAG" --verify-tag --draft');
    expect(publishJob).toContain('gh release upload "$TAG"');
    expect(publishJob).not.toContain('--clobber');
    expect(publishJob).not.toContain('tauri-action');
    expect(publishJob).not.toContain('npm run build');
    expect(publishJob).not.toContain('cargo ');
    const guardedPublish = publishJob.slice(publishJob.indexOf('Verify remote tag and publish guarded draft'));
    expect(guardedPublish).toContain('git ls-remote --tags origin');
    expect(guardedPublish).toContain('object !== process.env.TAG_OBJECT_SHA');
    expect(guardedPublish.match(/verify_remote_tag/g)).toHaveLength(3);
    expect(guardedPublish).toMatch(/verify_remote_tag[\s\S]*gh release edit "\$TAG" --draft=false[\s\S]*verify_remote_tag/);
    expect(releaseDocs).toContain('`CULL_RELEASE_PUBLISH_ENABLED` to equal `true`');
    expect(releaseDocs).toContain('must remain absent or false');
    expect(releaseDocs).toContain('29156442963');
    expect(releaseDocs).toContain('Apple notarization returned HTTP 403');
    expect(releaseDocs).toContain('Task 10 tag rules');
  });

  it('binds dispatch artifacts to the workflow invocation while evidence binds the selected tag commit', () => {
    const workflow = readFileSync(releaseWorkflowPath, 'utf8');
    const invocation = {
      workflowDispatchSha: '1111111111111111111111111111111111111111',
      inputTagCommit: '2222222222222222222222222222222222222222',
    };
    const verifyJob = workflowJob(workflow, 'verify-artifact');
    const publishJob = workflowJob(workflow, 'publish');

    expect(invocation.workflowDispatchSha).not.toBe(invocation.inputTagCommit);
    expect(verifyJob).toContain('EXPECTED_INVOCATION_SHA: ${{ github.sha }}');
    expect(publishJob).toContain('EXPECTED_INVOCATION_SHA: ${{ github.sha }}');
    expect(verifyJob).toContain('COMMIT: ${{ needs.release-gate.outputs.sha }}');
    expect(publishJob).toContain('COMMIT: ${{ needs.release-gate.outputs.sha }}');
  });

  it('validates public provenance and the exact DMG digest before exposing the tap token', () => {
    const workflow = readFileSync(tapWorkflowPath, 'utf8');
    const job = workflowJob(workflow, 'update-cask');

    expect(workflow).toMatch(/workflow_dispatch:\n\s+inputs:\n\s+version:[\s\S]*?dmg_sha256:[\s\S]*?provenance_url:/);
    expect(workflow.match(/required: true/g)?.length).toBeGreaterThanOrEqual(3);
    expect(job).toContain("schema !== 'cull.release.provenance.v1'");
    expect(job).toContain('provenance.version !== process.env.VERSION');
    expect(job).toContain('provenance.tag !== `v${process.env.VERSION}`');
    expect(job).toContain("!/^[0-9a-f]{40}$/.test(provenance.commit)");
    expect(job).toContain('Object.keys(provenance.assets).sort()');
    expect(job).toContain('Object.keys(provenance.checks).sort()');
    expect(job).toContain('provenance.checks[name] !== true');
    expect(job).toContain('provenance.assets[dmgName]?.sha256');
    expect(job).toContain('git ls-remote --tags');
    expect(job).toContain('provenance.tagObjectSha');
    expect(job).toContain('publicAsset.size !== provenanceAsset.size');
    expect(job).toContain('publicAsset.digest !== `sha256:${provenanceAsset.sha256}`');
    expect(job).toContain('checksums.txt');
    expect(job).toContain('CHECKSUMS_DIGEST_MISMATCH');
    expect(job).toContain('shasum -a 256');
    expect(job).toContain('DMG_SHA_MISMATCH');

    const verified = job.indexOf('Verify public provenance and DMG before tap access');
    const token = job.indexOf('${{ secrets.HOMEBREW_TAP_TOKEN }}');
    expect(verified).toBeGreaterThan(-1);
    expect(token).toBeGreaterThan(verified);
  });

  it('pins exactly version and SHA, rejects no_check, and makes equal promotion idempotent', () => {
    const workflow = readFileSync(tapWorkflowPath, 'utf8');
    const job = workflowJob(workflow, 'update-cask');

    expect(job).toContain('CASK_NO_CHECK');
    expect(job).toContain('node scripts/update-homebrew-cask.mjs');
    expect(job).toContain('CASK_DOWNGRADE');
    expect(job).toContain('CASK_IMMUTABLE_SHA_MISMATCH');
    expect(job).toContain('brew audit --cask cull');
    expect(job).toContain('--appdir="$RUNNER_TEMP/cull-apps"');
    expect(job).toContain('open -na "$app"');
    expect(job).toContain('git -C tap diff --exit-code');
    expect(job).not.toContain('sha256 :no_check\nsed');
    expect(job).not.toContain('git push --force');
    expect(job.indexOf('git -C tap commit')).toBeLessThan(job.indexOf('brew audit --cask cull'));
    expect(job.indexOf('brew audit --cask cull')).toBeLessThan(job.indexOf('git -C tap push'));
    expect(workflow).toContain('group: homebrew-cull');
    expect(workflow).not.toContain('group: homebrew-cull-${{');
  });

  it('requires launched Cull to remain alive before any tap push', () => {
    const workflow = readFileSync(tapWorkflowPath, 'utf8');
    const job = workflowJob(workflow, 'update-cask');
    const launch = job.indexOf('open -na "$app"');
    const liveness = job.indexOf('kill -0 "$app_pid"');
    const version = job.indexOf('CFBundleShortVersionString', launch);
    const push = job.indexOf('git -C tap push');
    expect(launch).toBeGreaterThan(-1);
    expect(liveness).toBeGreaterThan(launch);
    expect(version).toBeGreaterThan(liveness);
    expect(push).toBeGreaterThan(version);
  });

  it('dispatches Homebrew promotion from verified public provenance after publication', () => {
    const releaseWorkflow = readFileSync(releaseWorkflowPath, 'utf8');
    const publishJob = workflowJob(releaseWorkflow, 'publish');

    expect(publishJob).toContain('gh workflow run update-tap.yml');
    expect(publishJob).toContain('-f "version=$VERSION"');
    expect(publishJob).toContain('-f "dmg_sha256=$DMG_SHA256"');
    expect(publishJob).toContain('-f "provenance_url=$PROVENANCE_URL"');
    expect(publishJob.indexOf('gh release edit "$TAG" --draft=false'))
      .toBeLessThan(publishJob.indexOf('gh workflow run update-tap.yml'));
  });
});
