import { describe, expect, it } from 'vitest';
import { getCommandPaletteItems } from './command-palette';

describe('command palette AI library jobs', () => {
    it('exposes three explicit global jobs', () => {
        const items = getCommandPaletteItems('commands');
        expect(items.find(item => item.id === 'ai.detect-library')?.title).toBe('Detect Objects in Library');
        expect(items.find(item => item.id === 'ai.scan-sensitive-library')?.title).toBe('Scan Library for Sensitive Content');
        expect(items.find(item => item.id === 'ai.describe-library')?.title).toBe('Describe Images in Library');
    });
});
