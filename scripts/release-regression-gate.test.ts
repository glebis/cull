import { describe, expect, it, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import {
  regressionContracts,
  runRegressionContracts,
  validateRegressionContracts,
} from './release-regression-gate.mjs';

const requiredContracts = [
  'sidebar-search-filter',
  'settings-ai-reachable',
  'grid-deep-zoom-presets',
  'grid-hover-preview',
  'history-palette-deeplink',
  'thumbnail-prefetch',
];

describe('release regression gate', () => {
  it('names every release-critical user-visible behavior contract', () => {
    expect(regressionContracts.map((contract) => contract.id)).toEqual(requiredContracts);
    expect(regressionContracts.every((contract) => contract.tests.length > 0)).toBe(true);
  });

  it('rejects a missing behavior test with an actionable contract name', () => {
    expect(() => validateRegressionContracts(regressionContracts, (path) => (
      path !== 'src/lib/grid-hover-preview.test.ts'
    ))).toThrowError(expect.objectContaining({
      code: 'REGRESSION_CONTRACT_MISSING',
      message: expect.stringContaining('grid-hover-preview'),
      details: expect.objectContaining({ missingTests: ['src/lib/grid-hover-preview.test.ts'] }),
    }));
  });

  it('runs each behavior contract separately and stops at the named blocker', () => {
    const execute = vi.fn((contract: { id: string }) => ({
      status: contract.id === 'grid-hover-preview' ? 1 : 0,
      signal: null,
    }));

    expect(() => runRegressionContracts(regressionContracts, execute))
      .toThrowError(expect.objectContaining({
        code: 'REGRESSION_CONTRACT_FAILED',
        message: expect.stringContaining('grid-hover-preview'),
      }));
    expect(execute.mock.calls.map(([contract]) => contract.id)).toEqual([
      'sidebar-search-filter',
      'settings-ai-reachable',
      'grid-deep-zoom-presets',
      'grid-hover-preview',
    ]);
  });

  it('reports every named contract as passed when all behavior tests succeed', () => {
    const execute = vi.fn(() => ({ status: 0, signal: null }));

    expect(runRegressionContracts(regressionContracts, execute)).toEqual({
      ok: true,
      contracts: requiredContracts.map((id) => ({ id, status: 'passed' })),
    });
  });

  it('is blocking in local preparation and both signed release workflows', () => {
    const root = resolve(import.meta.dirname, '..');
    const packageJson = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf8'));
    const release = readFileSync(resolve(root, '.github/workflows/release.yml'), 'utf8');
    const canary = readFileSync(resolve(root, '.github/workflows/release-canary.yml'), 'utf8');

    expect(packageJson.scripts['test:release-regressions'])
      .toBe('node scripts/release-regression-gate.mjs');
    for (const workflow of [release, canary]) {
      expect(workflow).toContain('- name: Refresh verified origin/main');
      expect(workflow).toContain('git fetch --no-tags origin main');
      expect(workflow).toContain('- name: Run named release behavior regression gate');
      expect(workflow).toContain('run: npm run test:release-regressions');
      expect(workflow.indexOf('Run named release behavior regression gate'))
        .toBeLessThan(workflow.indexOf('Build unsigned frontend production bundle'));
    }
  });
});
