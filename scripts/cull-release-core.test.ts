import { describe, expect, it } from 'vitest';
import {
  RELEASE_STATES,
  buildResumeAction,
  classifyE2EPaths,
  createReleaseRecord,
  nextVersion,
  parseSemver,
  transitionReleaseRecord,
} from './cull-release-core.mjs';

describe('Cull release core', () => {
  it.each([
    ['0.2.5', 'patch', '0.2.6'],
    ['0.2.5', 'minor', '0.3.0'],
    ['0.2.5', 'major', '1.0.0'],
  ])('bumps %s as %s', (current, bump, expected) => {
    expect(nextVersion(current, bump)).toBe(expected);
  });

  it('rejects non-explicit SemVer', () => {
    expect(() => parseSemver('v0.2')).toThrow('Expected SemVer x.y.z');
  });

  it('classifies release E2E paths deterministically', () => {
    const rules = { exact: ['src/routes/+page.svelte'], prefixes: ['src/lib/components/'] };
    expect(classifyE2EPaths(['README.md'], rules)).toEqual([]);
    expect(classifyE2EPaths(['src/lib/components/Grid.svelte'], rules))
      .toEqual(['src/lib/components/Grid.svelte']);
  });

  it('allows only the next monotonic state', () => {
    const record = createReleaseRecord({
      version: '0.2.6', bump: 'patch', source: 'a'.repeat(40), now: '2026-07-10T12:00:00Z',
    });
    const checked = transitionReleaseRecord(
      record, 'checked', { check: 'release-gate.json' }, '2026-07-10T12:01:00Z',
    );
    expect(checked.state).toBe('checked');
    expect(() => transitionReleaseRecord(checked, 'published', {}, '2026-07-10T12:02:00Z'))
      .toThrow('Illegal release transition');
  });

  it('resumes without repeating an expensive verified build', () => {
    expect(buildResumeAction('artifact-verified')).toEqual({
      nextState: 'published', nextAction: 'publish-verified-artifacts',
    });
  });

  it('keeps the state list stable', () => {
    expect(RELEASE_STATES).toHaveLength(9);
  });
});
