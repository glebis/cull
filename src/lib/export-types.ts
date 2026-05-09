export interface ExportManifest {
    kind: string;
    schema_version: number;
    id: string;
    title: string;
    locale: string;
    created_at: string;
    updated_at: string;
    source: ManifestSource;
    defaults: ManifestDefaults;
    targets: ExportTarget[];
    slides: Slide[];
    assets: Asset[];
    agent_tasks: AgentTask[];
    agent_hints: AgentHints;
    agent_contract: AgentContract;
}

export interface ManifestSource {
    app: string;
    collection_id: string | null;
    image_ids: string[];
}

export interface ManifestDefaults {
    template: 'terminal' | 'editorial' | 'bleed';
    fonts: { serif: string; mono: string };
    colors: {
        preset: 'light' | 'dark';
        background: string;
        foreground: string;
        accent: string;
    };
    safe_area: { top: number; right: number; bottom: number; left: number };
}

export interface ExportTarget {
    id: string;
    platform: string;
    format: string;
    width: number;
    height: number;
    mime: string;
    quality?: number;
}

export interface Slide {
    id: string;
    template?: string;
    targets?: string[];
    image: {
        asset_id: string;
        fit: 'cover' | 'contain' | 'fill';
        focal_point?: { x: number; y: number };
    };
    text: {
        headline: string;
        body: string;
        caption: string;
    };
    overlay: {
        position: string;
        scrim: { type: string; direction: string; from: string; to: string };
        text_color: string;
    };
    metadata: {
        rating?: number;
        tags: string[];
        alt: string;
    };
}

export interface Asset {
    id: string;
    kind: 'source' | 'generated';
    uri: string;
    mime: string;
    width: number;
    height: number;
    provenance?: {
        provider: string;
        model: string;
        prompt: string;
        thinking?: boolean;
        reference_assets: string[];
        created_at: string;
    };
}

export interface AgentTask {
    slide_id: string;
    field: string;
    task: string;
    required: boolean;
    max_chars?: number;
}

export interface AgentHints {
    tone: string;
    allow_generated_images: boolean;
    language: string;
}

export interface AgentContract {
    mutable_paths: string[];
    append_only: string[];
    immutable_paths: string[];
}

export interface JsonPatch {
    op: 'replace' | 'add' | 'remove';
    path: string;
    value?: unknown;
}

export interface PatchResult {
    manifest: ExportManifest;
    applied_patches: JsonPatch[];
    rejected_patches: { patch: JsonPatch; reason: string }[];
}

export interface ValidationResult {
    valid: boolean;
    errors: { path: string; message: string }[];
    warnings: { path: string; message: string }[];
}

export interface PresetInfo {
    id: string;
    platform: string;
    format: string;
    width: number;
    height: number;
    mime: string;
}

export interface AssetResponse {
    path: string;
    mime: string;
    width: number;
    height: number;
}
