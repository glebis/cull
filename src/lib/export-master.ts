import type { ManifestDefaults } from './export-types';

export type ExportLayoutDensityId = 'basic' | 'standard' | 'detailed' | 'expert';
export type ExportSectionId =
    | 'outputs'
    | 'targets'
    | 'pdf-template'
    | 'text'
    | 'metadata'
    | 'advanced';
export type ExportOutputKey = 'originals' | 'web' | 'social' | 'pdf' | 'contact' | 'csv';
export type ExportIntentId =
    | 'client-delivery'
    | 'web-social'
    | 'portfolio-pdf'
    | 'contact-sheet'
    | 'archive-copy';
export type ExportTargetId =
    | 'web-responsive'
    | 'instagram-feed'
    | 'instagram-square'
    | 'stories-reels'
    | 'facebook-feed'
    | 'facebook-link'
    | 'linkedin-landscape'
    | 'linkedin-square'
    | 'portfolio-pdf';
export type PdfTemplateId = 'artistic' | 'photo' | 'lookbook' | 'contact' | 'custom';
export type PdfTextAmount = 'minimal' | 'standard' | 'extended';
export type PdfLayout = 'one image per page' | 'gallery grid' | 'sequence spreads' | 'cover + chapters';
export type SlideTemplate = ManifestDefaults['template'];

export interface ExportLayoutDensity {
    id: ExportLayoutDensityId;
    label: string;
    description: string;
    openSections: ExportSectionId[];
    hiddenSections: ExportSectionId[];
    showPromptDrawer: boolean;
}

export interface ExportIntent {
    id: ExportIntentId;
    label: string;
    prompt: string;
    outputs: ExportOutputKey[];
    targetId: ExportTargetId;
    pdfTemplateId?: PdfTemplateId;
    slideTemplate: SlideTemplate;
}

export interface ExportTargetOption {
    id: ExportTargetId;
    label: string;
    group: 'web' | 'social' | 'pdf';
    presetId: string;
    width: number;
    height: number;
    mime: string;
    note: string;
}

export interface PdfTemplateOption {
    id: PdfTemplateId;
    label: string;
    description: string;
    defaultTextAmount: PdfTextAmount;
    defaultLayout: PdfLayout;
    slideTemplate: SlideTemplate;
    htmlScaffold: string;
}

export interface ExportMasterSummaryState {
    intentId: ExportIntentId;
    targetId: ExportTargetId;
    outputs: ExportOutputKey[];
    pdfTemplateId: PdfTemplateId;
    pdfTextAmount: PdfTextAmount;
    pdfLayout: PdfLayout;
}

export const EXPORT_SECTION_IDS: ExportSectionId[] = [
    'outputs',
    'targets',
    'pdf-template',
    'text',
    'metadata',
    'advanced',
];

export const EXPORT_LAYOUT_DENSITIES: ExportLayoutDensity[] = [
    {
        id: 'basic',
        label: 'Basic',
        description: 'Key decisions only; advanced blocks stay out of the way.',
        openSections: ['outputs', 'targets'],
        hiddenSections: ['pdf-template', 'metadata', 'advanced'],
        showPromptDrawer: false,
    },
    {
        id: 'standard',
        label: 'Standard',
        description: 'Default working mode for delivery, web, social, and PDF exports.',
        openSections: ['outputs', 'targets', 'pdf-template', 'text'],
        hiddenSections: ['advanced'],
        showPromptDrawer: false,
    },
    {
        id: 'detailed',
        label: 'Detailed',
        description: 'More control over text, metadata, and template behavior.',
        openSections: ['outputs', 'targets', 'pdf-template', 'text', 'metadata'],
        hiddenSections: [],
        showPromptDrawer: false,
    },
    {
        id: 'expert',
        label: 'Expert',
        description: 'Everything visible, including prompt and HTML internals.',
        openSections: ['outputs', 'targets', 'pdf-template', 'text', 'metadata', 'advanced'],
        hiddenSections: [],
        showPromptDrawer: true,
    },
];

export const EXPORT_INTENTS: ExportIntent[] = [
    {
        id: 'client-delivery',
        label: 'Client delivery',
        prompt: 'Share polished web files with captions and a delivery index.',
        outputs: ['web', 'csv'],
        targetId: 'web-responsive',
        slideTemplate: 'editorial',
    },
    {
        id: 'web-social',
        label: 'Web and social set',
        prompt: 'Create export-ready assets for websites and social channels.',
        outputs: ['web', 'social', 'csv'],
        targetId: 'instagram-feed',
        slideTemplate: 'bleed',
    },
    {
        id: 'portfolio-pdf',
        label: 'Portfolio PDF',
        prompt: 'Build a composed PDF for portfolios, grants, pitches, or review decks.',
        outputs: ['pdf', 'web', 'csv'],
        targetId: 'portfolio-pdf',
        pdfTemplateId: 'artistic',
        slideTemplate: 'editorial',
    },
    {
        id: 'contact-sheet',
        label: 'Contact sheet',
        prompt: 'Review many images quickly with filenames, ratings, and short notes.',
        outputs: ['contact', 'pdf', 'csv'],
        targetId: 'portfolio-pdf',
        pdfTemplateId: 'contact',
        slideTemplate: 'terminal',
    },
    {
        id: 'archive-copy',
        label: 'Archive copy',
        prompt: 'Keep originals, metadata, and a CSV manifest together.',
        outputs: ['originals', 'csv'],
        targetId: 'web-responsive',
        slideTemplate: 'terminal',
    },
];

export const EXPORT_TARGET_OPTIONS: ExportTargetOption[] = [
    {
        id: 'web-responsive',
        label: 'Responsive web image',
        group: 'web',
        presetId: 'web_responsive',
        width: 1600,
        height: 1000,
        mime: 'image/jpeg',
        note: 'General website delivery at a balanced file size.',
    },
    {
        id: 'instagram-feed',
        label: 'Instagram feed portrait',
        group: 'social',
        presetId: 'ig_post',
        width: 1080,
        height: 1350,
        mime: 'image/png',
        note: 'Portrait feed and carousel frame.',
    },
    {
        id: 'instagram-square',
        label: 'Instagram square',
        group: 'social',
        presetId: 'ig_square',
        width: 1080,
        height: 1080,
        mime: 'image/png',
        note: 'Square post frame.',
    },
    {
        id: 'stories-reels',
        label: 'Stories and reels',
        group: 'social',
        presetId: 'ig_story',
        width: 1080,
        height: 1920,
        mime: 'image/png',
        note: 'Vertical story format for Instagram, Facebook, TikTok, and similar channels.',
    },
    {
        id: 'facebook-feed',
        label: 'Facebook feed',
        group: 'social',
        presetId: 'fb_feed',
        width: 1080,
        height: 1350,
        mime: 'image/jpeg',
        note: 'Portrait feed image for Facebook.',
    },
    {
        id: 'facebook-link',
        label: 'Facebook link preview',
        group: 'social',
        presetId: 'fb_link',
        width: 1200,
        height: 630,
        mime: 'image/jpeg',
        note: 'Open Graph preview size for shared links.',
    },
    {
        id: 'linkedin-landscape',
        label: 'LinkedIn landscape',
        group: 'social',
        presetId: 'li_post',
        width: 1200,
        height: 628,
        mime: 'image/jpeg',
        note: 'Landscape update and link-friendly frame.',
    },
    {
        id: 'linkedin-square',
        label: 'LinkedIn square',
        group: 'social',
        presetId: 'li_square',
        width: 1200,
        height: 1200,
        mime: 'image/png',
        note: 'Square post frame for feed visibility.',
    },
    {
        id: 'portfolio-pdf',
        label: 'Portfolio PDF',
        group: 'pdf',
        presetId: 'portfolio_pdf',
        width: 2480,
        height: 3508,
        mime: 'application/pdf',
        note: 'A4 portrait page at 300 DPI for portfolios and lookbooks.',
    },
];

export const PDF_TEMPLATE_OPTIONS: PdfTemplateOption[] = [
    {
        id: 'artistic',
        label: 'Artistic portfolio',
        description: 'Statement-led sequence with generous image pacing and essay text.',
        defaultTextAmount: 'extended',
        defaultLayout: 'sequence spreads',
        slideTemplate: 'editorial',
        htmlScaffold:
            '<section class="cover"><h1>{{portfolio_title}}</h1><p>{{artist_statement}}</p></section>\n<section class="work">{{image_sequence}}</section>\n<section class="notes">{{process_notes}}</section>',
    },
    {
        id: 'photo',
        label: 'Photo portfolio',
        description: 'Client or project narrative with chapters, captions, and delivery notes.',
        defaultTextAmount: 'standard',
        defaultLayout: 'cover + chapters',
        slideTemplate: 'bleed',
        htmlScaffold:
            '<section class="cover"><h1>{{client_or_project}}</h1><p>{{project_summary}}</p></section>\n<section class="chapter">{{chapter_title}}</section>\n<section class="gallery">{{captioned_images}}</section>',
    },
    {
        id: 'lookbook',
        label: 'Minimal lookbook',
        description: 'Quiet image-first pages with sparse captions.',
        defaultTextAmount: 'minimal',
        defaultLayout: 'one image per page',
        slideTemplate: 'bleed',
        htmlScaffold:
            '<section class="lookbook-cover"><h1>{{title}}</h1></section>\n<section class="plate">{{image}}<p>{{short_caption}}</p></section>',
    },
    {
        id: 'contact',
        label: 'Contact-sheet booklet',
        description: 'Dense proofing pages with identifiers, ratings, and selection notes.',
        defaultTextAmount: 'minimal',
        defaultLayout: 'gallery grid',
        slideTemplate: 'terminal',
        htmlScaffold:
            '<section class="contact-sheet"><h1>{{set_name}}</h1><div class="grid">{{thumbnails_with_metadata}}</div></section>',
    },
    {
        id: 'custom',
        label: 'Custom HTML template',
        description: 'Start from editable HTML and keep the export preset under your control.',
        defaultTextAmount: 'standard',
        defaultLayout: 'sequence spreads',
        slideTemplate: 'editorial',
        htmlScaffold:
            '<main class="portfolio">{{custom_cover}}{{custom_sections}}{{image_sequence}}</main>',
    },
];

export function getExportLayoutDensity(id: ExportLayoutDensityId): ExportLayoutDensity {
    return EXPORT_LAYOUT_DENSITIES.find(density => density.id === id) ?? EXPORT_LAYOUT_DENSITIES[0];
}

export function getExportIntent(id: ExportIntentId): ExportIntent {
    return EXPORT_INTENTS.find(intent => intent.id === id) ?? EXPORT_INTENTS[0];
}

export function getExportTargetOption(id: ExportTargetId): ExportTargetOption {
    return EXPORT_TARGET_OPTIONS.find(target => target.id === id) ?? EXPORT_TARGET_OPTIONS[0];
}

export function getPdfTemplateOption(id: PdfTemplateId): PdfTemplateOption {
    return PDF_TEMPLATE_OPTIONS.find(template => template.id === id) ?? PDF_TEMPLATE_OPTIONS[0];
}

export function hasPdfOutput(outputs: ExportOutputKey[]): boolean {
    return outputs.includes('pdf');
}

export function describePdfTextAmount(amount: PdfTextAmount): string {
    if (amount === 'minimal') return 'minimal captions';
    if (amount === 'extended') return 'extended statements';
    return 'standard notes';
}

export function buildExportMasterSummary(state: ExportMasterSummaryState): string {
    const intent = getExportIntent(state.intentId);
    const target = getExportTargetOption(state.targetId);
    const parts = [
        intent.label,
        target.label,
        `${target.width}×${target.height}`,
        state.outputs.join(', '),
    ];

    if (hasPdfOutput(state.outputs)) {
        const pdfTemplate = getPdfTemplateOption(state.pdfTemplateId);
        parts.push(`${pdfTemplate.label}, ${describePdfTextAmount(state.pdfTextAmount)}, ${state.pdfLayout}`);
    }

    return parts.join(' · ');
}
