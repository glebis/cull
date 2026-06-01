import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
    createWorkflow,
    deleteWorkflow,
    isDestructiveCommand,
    readWorkflows,
    renameWorkflow,
    runWorkflow,
    workflowId,
    type CommandWorkflow,
} from './workflows';
import { getCommandPaletteItems, type CommandPaletteItem } from './command-palette';

function stubStorage() {
    const store = new Map<string, string>();
    (globalThis as { localStorage?: unknown }).localStorage = {
        getItem: (k: string) => (store.has(k) ? store.get(k)! : null),
        setItem: (k: string, v: string) => void store.set(k, String(v)),
        removeItem: (k: string) => void store.delete(k),
        clear: () => store.clear(),
    };
}

function cmd(id: string, extra: Partial<CommandPaletteItem> = {}): CommandPaletteItem {
    return {
        id,
        title: id,
        category: 'Test',
        kind: 'command',
        run: () => undefined,
        ...extra,
    };
}

describe('workflow storage CRUD', () => {
    beforeEach(stubStorage);

    it('creates, renames, and deletes workflows persistently', () => {
        const wf = createWorkflow('Prepare Delivery', ['view.grid', 'collection.create-from-selection']);
        expect(wf.id).toBe('workflow.prepare-delivery');
        expect(readWorkflows()).toHaveLength(1);

        renameWorkflow(wf.id, 'Delivery Set');
        expect(readWorkflows()[0].name).toBe('Delivery Set');

        deleteWorkflow(wf.id);
        expect(readWorkflows()).toHaveLength(0);
    });

    it('generates collision-free ids for duplicate names', () => {
        const existing: CommandWorkflow[] = [{ id: 'workflow.review', name: 'Review', steps: [] }];
        expect(workflowId('Review', existing)).toBe('workflow.review-2');
    });

    it('classifies destructive commands', () => {
        expect(isDestructiveCommand('image.trash')).toBe(true);
        expect(isDestructiveCommand('image.delete-permanently')).toBe(true);
        expect(isDestructiveCommand('view.grid')).toBe(false);
    });
});

describe('workflow execution', () => {
    beforeEach(stubStorage);

    it('runs each step in order through the registry', async () => {
        const order: string[] = [];
        const items: Record<string, CommandPaletteItem> = {
            a: cmd('a', { run: () => void order.push('a') }),
            b: cmd('b', { run: () => void order.push('b') }),
        };
        const result = await runWorkflow({ id: 'w', name: 'W', steps: ['a', 'b'] }, {
            resolveItem: id => items[id],
        });
        expect(result.ok).toBe(true);
        expect(result.completed).toBe(2);
        expect(order).toEqual(['a', 'b']);
    });

    it('halts on an unknown command id', async () => {
        const result = await runWorkflow({ id: 'w', name: 'W', steps: ['ghost'] }, {
            resolveItem: () => null,
        });
        expect(result.ok).toBe(false);
        expect(result.error).toContain('Unknown command');
        expect(result.completed).toBe(0);
    });

    it('halts when a step is unavailable in the current context', async () => {
        const result = await runWorkflow({ id: 'w', name: 'W', steps: ['a'] }, {
            resolveItem: () => cmd('a', { disabled: true }),
        });
        expect(result.ok).toBe(false);
        expect(result.error).toContain('unavailable');
    });

    it('confirms destructive steps and aborts cleanly when declined', async () => {
        const confirm = vi.fn().mockResolvedValue(false);
        const result = await runWorkflow({ id: 'w', name: 'W', steps: ['image.trash'] }, {
            resolveItem: () => cmd('image.trash'),
            confirm,
        });
        expect(confirm).toHaveBeenCalledOnce();
        expect(result.ok).toBe(false);
        expect(result.cancelled).toBe(true);
        expect(result.completed).toBe(0);
    });

    it('stops with an error message when a step throws', async () => {
        const result = await runWorkflow({ id: 'w', name: 'W', steps: ['a', 'b'] }, {
            resolveItem: id => (id === 'a'
                ? cmd('a')
                : cmd('b', { run: () => { throw new Error('boom'); } })),
        });
        expect(result.ok).toBe(false);
        expect(result.completed).toBe(1);
        expect(result.error).toContain('failed');
    });
});

describe('workflow palette integration', () => {
    beforeEach(stubStorage);

    it('always offers the Save Workflow command', () => {
        const ids = getCommandPaletteItems('commands').map(i => i.id);
        expect(ids).toContain('workflow.create-from-recents');
    });

    it('surfaces saved workflows as runnable, pinnable command items', () => {
        const wf = createWorkflow('Review Import', ['view.grid']);
        const items = getCommandPaletteItems('commands');
        const workflowItem = items.find(i => i.id === wf.id);
        expect(workflowItem).toBeTruthy();
        expect(workflowItem?.kind).toBe('command');
        expect(workflowItem?.category).toBe('Workflow');
        expect(workflowItem?.subtitle).toContain('1 step');
    });
});
