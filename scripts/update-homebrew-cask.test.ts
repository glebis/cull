import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const script = resolve(import.meta.dirname, 'update-homebrew-cask.mjs');
const sha = (character: string) => character.repeat(64);

function execute(source: string, version = '1.2.4', digest = sha('a')) {
  const root = mkdtempSync(join(tmpdir(), 'cull-cask-update-'));
  const cask = join(root, 'cull.rb');
  writeFileSync(cask, source);
  const result = spawnSync(process.execPath, [
    script, '--cask', cask, '--version', version, '--sha256', digest, '--json',
  ], { encoding: 'utf8' });
  return {
    ...result,
    output: JSON.parse(result.stdout),
    contents: readFileSync(cask, 'utf8'),
  };
}

describe('Homebrew cask release editor', () => {
  it('rejects no_check even with a trailing comment', () => {
    const source = 'version "1.2.3"\nsha256 :no_check # temporary\n';
    const result = execute(source);
    expect(result.status).toBe(2);
    expect(result.output.code).toBe('CASK_NO_CHECK');
    expect(result.contents).toBe(source);
  });

  it.each([
    ['version', 'version "1.2.3" # first\nversion "1.2.2" # duplicate\nsha256 "' + sha('b') + '"\n'],
    ['sha', 'version "1.2.3"\nsha256 "' + sha('b') + '" # first\nsha256 "' + sha('c') + '" # duplicate\n'],
  ])('rejects duplicate active %s directives including commented lines', (_name, source) => {
    const result = execute(source);
    expect(result.status).toBe(2);
    expect(result.output.code).toBe('CASK_INVALID');
    expect(result.contents).toBe(source);
  });

  it('rejects a SemVer downgrade without writing', () => {
    const source = `version "2.0.0"\nsha256 "${sha('b')}"\n`;
    const result = execute(source, '1.9.9');
    expect(result.status).toBe(2);
    expect(result.output.code).toBe('CASK_DOWNGRADE');
    expect(result.contents).toBe(source);
  });

  it('rejects changing the SHA of an already-published equal version', () => {
    const source = `version "1.2.4"\nsha256 "${sha('b')}"\n`;
    const result = execute(source, '1.2.4', sha('a'));
    expect(result.status).toBe(2);
    expect(result.output.code).toBe('CASK_IMMUTABLE_SHA_MISMATCH');
    expect(result.contents).toBe(source);
  });

  it('returns an idempotent no-write result only for equal version and SHA', () => {
    const source = `version "1.2.4"\nsha256 "${sha('a')}"\n`;
    const result = execute(source);
    expect(result.status).toBe(0);
    expect(result.output.result).toEqual({ changed: false, previousVersion: '1.2.4' });
    expect(result.contents).toBe(source);
  });

  it('updates exactly the sole canonical version and SHA directives for an upgrade', () => {
    const source = `cask "cull" do\n  version "1.2.3"\n  sha256 "${sha('b')}"\n  url "https://example.test/Cull.dmg"\nend\n`;
    const result = execute(source);
    expect(result.status).toBe(0);
    expect(result.output.result).toEqual({ changed: true, previousVersion: '1.2.3' });
    expect(result.contents).toBe(`cask "cull" do\n  version "1.2.4"\n  sha256 "${sha('a')}"\n  url "https://example.test/Cull.dmg"\nend\n`);
  });
});
