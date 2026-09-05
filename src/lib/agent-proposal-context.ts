import type { AgentPersona } from '$lib/api';

export type AgentProposalViewContext = {
    kind: string;
    id: string | null;
    label: string;
    path: string | null;
    view_mode?: string | null;
    selected_count?: number;
    visible_count?: number;
    /** Captured Selection Mode run target. Written at the top level of
     *  view_context_json while a run is active so the backend bridge can pin
     *  shortlist proposals to that exact run instead of whichever run happens
     *  to be active at approval time. */
    selection_id?: string | null;
};

export type AgentProposalActorContext = {
    type?: string | null;
    name?: string | null;
    role?: string | null;
    token_id?: string | null;
};

export type AgentProposalSourceContext = {
    source?: string | null;
    selected_count?: number;
    visible_count?: number;
    candidate_count?: number;
    active_preset_id?: string | null;
    scope_key?: string | null;
    scope_label?: string | null;
    actor?: AgentProposalActorContext | null;
    view_context?: AgentProposalViewContext | null;
    [key: string]: unknown;
};

export function parseAgentProposalSourceContext(sourceJson: string | null | undefined): AgentProposalSourceContext {
    if (!sourceJson) return {};
    try {
        const parsed = JSON.parse(sourceJson);
        return parsed && typeof parsed === 'object' && !Array.isArray(parsed)
            ? parsed as AgentProposalSourceContext
            : {};
    } catch {
        return {};
    }
}

export function proposalViewContextKey(viewContext: AgentProposalViewContext | null | undefined): string | null {
    if (!viewContext?.kind) return null;
    const discriminator = viewContext.kind === 'folder'
        ? viewContext.path
        : viewContext.id ?? viewContext.path ?? viewContext.label;
    const mode = viewContext.view_mode ? `@${viewContext.view_mode}` : '';
    return discriminator ? `${viewContext.kind}:${discriminator}${mode}` : `${viewContext.kind}${mode}`;
}

export function sourceContextViewContext(context: AgentProposalSourceContext): AgentProposalViewContext | null {
    return isViewContext(context.view_context) ? context.view_context : null;
}

export function sourceContextScopeKey(context: AgentProposalSourceContext): string | null {
    if (typeof context.scope_key === 'string' && context.scope_key.trim()) {
        return context.scope_key.trim();
    }
    return proposalViewContextKey(sourceContextViewContext(context));
}

export function sourceContextScopeLabel(context: AgentProposalSourceContext): string | null {
    const viewContext = sourceContextViewContext(context);
    if (typeof context.scope_label === 'string' && context.scope_label.trim()) {
        return withViewMode(context.scope_label.trim(), viewContext);
    }
    return viewContext ? withViewMode(viewContext.label, viewContext) : null;
}

export function sourceContextIsStale(
    context: AgentProposalSourceContext,
    currentViewContext: AgentProposalViewContext | null | undefined,
): boolean {
    const sourceKey = sourceContextScopeKey(context);
    const currentKey = proposalViewContextKey(currentViewContext);
    return Boolean(sourceKey && currentKey && sourceKey !== currentKey);
}

// ---------------------------------------------------------------------------
// Shortlist proposals (Selection Mode)
// ---------------------------------------------------------------------------

/** Proposal kinds that mutate Selection Mode shortlist membership. They are
 *  applied only through the proposal apply endpoint against the run captured
 *  in source_context_json.selection_id. */
export const SHORTLIST_PROPOSAL_KINDS = ['shortlist_add', 'shortlist_remove'] as const;

export function isShortlistProposalKind(kind: string): boolean {
    return (SHORTLIST_PROPOSAL_KINDS as readonly string[]).includes(kind);
}

/** The Selection Mode run a shortlist proposal targets, captured when the
 *  proposal was created — never inferred from the currently open run. */
export function sourceContextSelectionId(context: AgentProposalSourceContext): string | null {
    const value = context.selection_id;
    return typeof value === 'string' && value.trim() ? value.trim() : null;
}

/** Button label for applying an approved subset, distinct from highlight
 *  ("Select approved") and trash ("Move approved to Trash") actions. */
export function shortlistProposalActionLabel(kind: string): string {
    return kind === 'shortlist_remove'
        ? 'Remove approved from shortlist'
        : 'Add approved to shortlist';
}

/** Short human kind label used in the dock switcher and dialog titles. */
export function proposalKindLabel(kind: string): string {
    if (kind === 'trash_images') return 'Trash';
    if (isShortlistProposalKind(kind)) return 'Shortlist';
    return 'Selection';
}

export function proposalActorLabel(context: AgentProposalSourceContext, fallbackPersona: AgentPersona): string {
    const actor = isActorContext(context.actor) ? context.actor : null;
    const actorName = actor?.name?.trim();
    if (actorName) {
        const role = actor?.role?.trim();
        return role ? `${actorName} (${role})` : actorName;
    }

    const source = typeof context.source === 'string' ? context.source : '';
    if (source.includes('claude')) return `Claude (${fallbackPersona})`;
    if (source === 'agent_chat_manual_seed') return `Cull UI (${fallbackPersona})`;
    if (source.startsWith('plugin:')) return `Plugin ${source.slice('plugin:'.length)}`;
    if (source.trim()) return `${humanizeSource(source)} (${fallbackPersona})`;
    return fallbackPersona;
}

function isViewContext(value: unknown): value is AgentProposalViewContext {
    if (!value || typeof value !== 'object' || Array.isArray(value)) return false;
    const record = value as Record<string, unknown>;
    return typeof record.kind === 'string' && typeof record.label === 'string';
}

function isActorContext(value: unknown): value is AgentProposalActorContext {
    return Boolean(value && typeof value === 'object' && !Array.isArray(value));
}

function humanizeSource(source: string): string {
    return source
        .split(/[_:.-]+/)
        .filter(Boolean)
        .map(part => `${part.charAt(0).toUpperCase()}${part.slice(1)}`)
        .join(' ');
}

function withViewMode(label: string, viewContext: AgentProposalViewContext | null): string {
    const mode = viewContext?.view_mode?.trim();
    if (!mode || label.includes(`(${mode})`)) return label;
    return `${label} (${mode})`;
}
