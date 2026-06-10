import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const exportSource = readFileSync(join(process.cwd(), 'src/lib/components/Export.svelte'), 'utf8');

describe('Export Master UI contract', () => {
    it('renders the final export master structure instead of the legacy compact toolbar', () => {
        expect(exportSource).toContain('class="export-master"');
        expect(exportSource).toContain('Layout density');
        expect(exportSource).toContain('What are you making?');
        expect(exportSource).toContain('class="answer-grid"');
        expect(exportSource).toContain('class="export-queue"');
    });

    it('contains collapsible blocks for outputs, target formats, PDF templates, per-image text, metadata, and advanced controls', () => {
        expect(exportSource).toContain('data-section="outputs"');
        expect(exportSource).toContain('data-section="targets"');
        expect(exportSource).toContain('data-section="pdf-template"');
        expect(exportSource).toContain('data-section="text"');
        expect(exportSource).toContain('data-section="metadata"');
        expect(exportSource).toContain('data-section="advanced"');
    });

    it('keeps the existing render and export pipeline wired through manifest slides', () => {
        expect(exportSource).toContain('createExportManifest');
        expect(exportSource).toContain('toPng');
        expect(exportSource).toContain('save_export_image');
        expect(exportSource).toContain('assemble_export_pdf');
        expect(exportSource).toContain('bind:this={renderRefs[slide.id]}');
    });
});
