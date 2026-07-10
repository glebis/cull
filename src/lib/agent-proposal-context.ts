import type { AgentPersona } from '$lib/api';

export type AgentProposalViewContext = {
    kind: string;
    id: string | null;
    label: string;
    path: string | null;
    view_mode?: string | null;
    selected_count?: number;
    visible_count?: number;
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
