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
  git(root, 'tag', 'v1.2.3', baseSha);

  metadata(root, '1.2.4');
  write(root, 'CHANGELOG.md', '# Changelog\n\n## [1.2.4] - 2026-07-11\n\n### Fixed\n\n- Safe release gates.\n\n## [1.2.3] - 2026-07-01\n');
  write(root, 'docs/COMPATIBILITY.md', 'Last updated: 1.2.4 (2026-07-11)\n');
  write(root, 'src/lib/api.ts', `export const version = 2;\n${stableApiBody}\n`);
  const sha = commit(root, 'release');
  git(root, 'tag', 'v1.2.4', sha);
  git(root, 'update-ref', 'refs/remotes/origin/main', sha);
  return { root, baseSha, sha };
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
    expect(output).toEqual({
      schema: 'cull.release.gate.v1',
      version: '1.2.4',
      tag: 'v1.2.4',
      sha,
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

  it('rejects a release SHA that is not reachable from origin/main', () => {
    const { root, baseSha } = fixture();
    git(root, 'switch', '--detach', baseSha);
    metadata(root, '1.2.4');
    write(root, 'CHANGELOG.md', '# Changelog\n\n## [1.2.4] - 2026-07-11\n');
    write(root, 'docs/COMPATIBILITY.md', 'Last updated: 1.2.4 (2026-07-11)\n');
    const divergent = commit(root, 'divergent release');
    git(root, 'tag', 'v1.2.4-divergent', divergent);
    git(root, 'tag', '-f', 'v1.2.4', divergent);

    expectRejected(run(root, { sha: divergent }), 'NOT_ON_ORIGIN_MAIN');
  });

  it('classifies the covered source of a rename even when Git rename detection is enabled', () => {
    const { root } = fixture();
    mkdirSync(join(root, 'docs'), { recursive: true });
    renameSync(join(root, 'src/lib/api.ts'), join(root, 'docs/renamed-api.ts'));
    const sha = commit(root, 'rename covered source outside E2E policy');
    git(root, 'tag', '-f', 'v1.2.4', sha);
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
    git(root, 'tag', '-f', 'v1.2.4', mismatched);
    git(root, 'update-ref', 'refs/remotes/origin/main', mismatched);
    expect(released).not.toBe(mismatched);

    expectRejected(run(root, { sha: mismatched }), 'VERSION_MISMATCH');
  });

  it('rejects a missing changelog stamp for the version', () => {
    const { root } = fixture();
    write(root, 'CHANGELOG.md', '# Changelog\n\n## [1.2.3] - 2026-07-01\n');
    const sha = commit(root, 'missing changelog stamp');
    git(root, 'tag', '-f', 'v1.2.4', sha);
    git(root, 'update-ref', 'refs/remotes/origin/main', sha);

    expectRejected(run(root, { sha }), 'CHANGELOG_INVALID');
  });

  it('rejects release configuration missing a stable contract command', () => {
    const { root } = fixture();
    config(root, [DB_CONTRACT]);
    const sha = commit(root, 'missing stable export gate');
    git(root, 'tag', '-f', 'v1.2.4', sha);
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
    expect(gateJob).toContain("if: steps.release_gate.outputs.e2e_required == 'true'");
    expect(gateJob).toContain('bash tests/e2e/run-e2e.sh');
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
  });
});
