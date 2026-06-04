import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const modalDialog = readFileSync(join(process.cwd(), 'src/lib/components/ModalDialog.svelte'), 'utf8');
const trashDialog = readFileSync(join(process.cwd(), 'src/lib/components/TrashConfirmDialog.svelte'), 'utf8');

describe('modal dialog accessibility contract', () => {
    it('keeps key events scoped to the modal shell', () => {
        const lines = modalDialog.split('\n');
        expect(lines.some(line => line.includes('event.stopPropagation();'))).toBe(true);
    });

    it('prevents nested overlay close events from bubbling', () => {
        expect(modalDialog).toContain('function handleOverlayClick(event: MouseEvent)');
        expect(modalDialog).toContain('onkeydown={handleKeydown}');
        expect(modalDialog).toContain('onclick={handleOverlayClick}');
    });

    it('does not bind trash confirmation to bare Enter keyboard handling', () => {
        expect(trashDialog).not.toContain('event.key === \'Enter\'');
    });
});
