// Saved command workflows: user-defined sequences of registered command IDs.
// Deliberately NOT arbitrary scripts — every step is an existing palette command,
// so workflows reuse the registry's context predicates, disabled state, and runners.

import { isCommandPaletteItemVisible, runCommandPaletteItem, type CommandPaletteItem } from './command-palette';

export interface CommandWorkflow {
    id: string;
    name: string;
    steps: string[];
}

export const COMMAND_WORKFLOWS_STORAGE_KEY = 'cull.commandPalette.workflows';

// Command IDs whose steps mutate or remove data and should prompt before running
// as part of an unattended sequence.
const DESTRUCTIVE_PREFIXES = ['image.trash', 'image.delete'];

export function isDestructiveCommand(commandId: string): boolean {
    return DESTRUCTIVE_PREFIXES.some(prefix => commandId.startsWith(prefix));
}

function storageAvailable(): boolean {
    return typeof localStorage !== 'undefined';
}

export function readWorkflows(): CommandWorkflow[] {
    if (!storageAvailable()) return [];
    try {
        const raw = localStorage.getItem(COMMAND_WORKFLOWS_STORAGE_KEY);
        const parsed = raw ? JSON.parse(raw) as CommandWorkflow[] : [];
        return Array.isArray(parsed) ? parsed.filter(wf => wf && wf.id && Array.isArray(wf.steps)) : [];
    } catch {
        return [];
    }
}

function writeWorkflows(workflows: CommandWorkflow[]) {
    if (!storageAvailable()) return;
    localStorage.setItem(COMMAND_WORKFLOWS_STORAGE_KEY, JSON.stringify(workflows));
}

function slugify(name: string): string {
    return name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '') || 'workflow';
}

// Stable, collision-free ID derived from the name so tests stay deterministic
// (no reliance on Date.now / random).
export function workflowId(name: string, existing: CommandWorkflow[]): string {
    const base = `workflow.${slugify(name)}`;
    if (!existing.some(wf => wf.id === base)) return base;
    let n = 2;
    while (existing.some(wf => wf.id === `${base}-${n}`)) n += 1;
    return `${base}-${n}`;
}

export function createWorkflow(name: string, steps: string[]): CommandWorkflow {
    const trimmed = name.trim();
    const existing = readWorkflows();
    const workflow: CommandWorkflow = {
        id: workflowId(trimmed, existing),
        name: trimmed,
        steps: steps.filter(Boolean),
    };
    writeWorkflows([...existing, workflow]);
    return workflow;
}

export function renameWorkflow(id: string, name: string): CommandWorkflow[] {
    const next = readWorkflows().map(wf => (wf.id === id ? { ...wf, name: name.trim() } : wf));
    writeWorkflows(next);
    return next;
}

export function deleteWorkflow(id: string): CommandWorkflow[] {
    const next = readWorkflows().filter(wf => wf.id !== id);
    writeWorkflows(next);
    return next;
}

export interface RunWorkflowContext {
    // Resolve a command ID to its current live palette item (with up-to-date
    // disabled/visible state).
    resolveItem: (commandId: string) => CommandPaletteItem | null | undefined;
    // Confirm a destructive step. Return false to abort the workflow.
    confirm?: (item: CommandPaletteItem) => boolean | Promise<boolean>;
}

export interface WorkflowRunResult {
    ok: boolean;
    completed: number;
    total: number;
    // Reason the run halted early, if any. Undefined means every step ran.
    error?: string;
    // True when the user declined a destructive confirmation (clean stop).
    cancelled?: boolean;
}

// Run a workflow step-by-step. Validates context before each step, confirms
// destructive steps, and halts on the first failure without leaving the caller
// guessing — the result reports exactly how far it got and why it stopped.
export async function runWorkflow(workflow: CommandWorkflow, ctx: RunWorkflowContext): Promise<WorkflowRunResult> {
    const total = workflow.steps.length;
    let completed = 0;

    for (const stepId of workflow.steps) {
        const item = ctx.resolveItem(stepId);
        if (!item) {
            return { ok: false, completed, total, error: `Unknown command: ${stepId}` };
        }
        if (!isCommandPaletteItemVisible(item) || item.disabled) {
            return { ok: false, completed, total, error: `Step unavailable in this context: ${item.title}` };
        }
        if (isDestructiveCommand(item.id) && ctx.confirm) {
            const proceed = await ctx.confirm(item);
            if (!proceed) {
                return { ok: false, completed, total, cancelled: true };
            }
        }
        try {
            await runCommandPaletteItem(item);
        } catch (err) {
            return { ok: false, completed, total, error: `${item.title} failed: ${err}` };
        }
        completed += 1;
    }

    return { ok: true, completed, total };
}
