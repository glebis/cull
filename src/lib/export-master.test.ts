import { describe, expect, it } from 'vitest';
import {
    EXPORT_INTENTS,
    EXPORT_LAYOUT_DENSITIES,
    EXPORT_TARGET_OPTIONS,
    PDF_TEMPLATE_OPTIONS,
    buildExportMasterSummary,
    getExportIntent,
    getExportTargetOption,
    getPdfTemplateOption,
    hasPdfOutput,
} from './export-master';

describe('export master model', () => {
    it('defines density presets that progressively reveal export controls', () => {
        expect(EXPORT_LAYOUT_DENSITIES.map(density => density.id)).toEqual([
            'basic',
            'standard',
            'detailed',
            'expert',
        ]);
        expect(EXPORT_LAYOUT_DENSITIES.find(density => density.id === 'basic')?.hiddenSections).toContain('pdf-template');
        expect(EXPORT_LAYOUT_DENSITIES.find(density => density.id === 'standard')?.openSections).toContain('pdf-template');
        expect(EXPORT_LAYOUT_DENSITIES.find(density => density.id === 'expert')?.showPromptDrawer).toBe(true);
    });

    it('models the final export intents including PDF portfolios and web/social sets', () => {
        expect(EXPORT_INTENTS.map(intent => intent.id)).toContain('portfolio-pdf');
        expect(EXPORT_INTENTS.map(intent => intent.id)).toContain('web-social');

        const portfolio = getExportIntent('portfolio-pdf');
        expect(portfolio.outputs).toContain('pdf');
        expect(portfolio.pdfTemplateId).toBe('artistic');
        expect(portfolio.targetId).toBe('portfolio-pdf');

        const social = getExportIntent('web-social');
        expect(social.outputs).toEqual(['web', 'social', 'csv']);
        expect(social.targetId).toBe('instagram-feed');
    });

    it('maps social and PDF targets to backend export preset ids', () => {
        expect(EXPORT_TARGET_OPTIONS.map(target => target.id)).toEqual([
            'web-responsive',
            'instagram-feed',
            'instagram-square',
            'stories-reels',
            'facebook-feed',
            'facebook-link',
            'linkedin-landscape',
            'linkedin-square',
            'portfolio-pdf',
        ]);
        expect(getExportTargetOption('linkedin-landscape')).toMatchObject({
            presetId: 'li_post',
            width: 1200,
            height: 628,
        });
        expect(getExportTargetOption('linkedin-square')).toMatchObject({
            presetId: 'li_square',
            width: 1200,
            height: 1200,
        });
        expect(getExportTargetOption('portfolio-pdf')).toMatchObject({
            presetId: 'portfolio_pdf',
            mime: 'application/pdf',
        });
    });

    it('defines editable PDF HTML template defaults', () => {
        expect(PDF_TEMPLATE_OPTIONS.map(template => template.id)).toEqual([
            'artistic',
            'photo',
            'lookbook',
            'contact',
            'custom',
        ]);

        const artistic = getPdfTemplateOption('artistic');
        expect(artistic.label).toBe('Artistic portfolio');
        expect(artistic.htmlScaffold).toContain('{{artist_statement}}');

        const photo = getPdfTemplateOption('photo');
        expect(photo.label).toBe('Photo portfolio');
        expect(photo.defaultLayout).toBe('cover + chapters');
        expect(photo.htmlScaffold).toContain('{{client_or_project}}');
    });

    it('summarizes PDF template state only when PDF output is enabled', () => {
        expect(hasPdfOutput(['web', 'social', 'csv'])).toBe(false);
        expect(hasPdfOutput(['web', 'pdf', 'csv'])).toBe(true);

        expect(buildExportMasterSummary({
            intentId: 'portfolio-pdf',
            targetId: 'portfolio-pdf',
            outputs: ['web', 'pdf', 'csv'],
            pdfTemplateId: 'photo',
            pdfTextAmount: 'extended',
            pdfLayout: 'cover + chapters',
        })).toContain('Photo portfolio, extended statements, cover + chapters');

        expect(buildExportMasterSummary({
            intentId: 'web-social',
            targetId: 'instagram-feed',
            outputs: ['web', 'social', 'csv'],
            pdfTemplateId: 'photo',
            pdfTextAmount: 'extended',
            pdfLayout: 'cover + chapters',
        })).not.toContain('Photo portfolio');
    });
});
