import { describe, expect, it, vi } from 'vitest';
import { registerCanvasPathMigrationParticipant, withCanvasPathMigrationBarrier } from './canvas-save-coordinator';

describe('canvas save coordinator', () => {
    it('holds the active canvas barrier around the migration and unregisters it safely', async () => {
        const calls: string[] = [];
        const unregister = registerCanvasPathMigrationParticipant(
            async () => { calls.push('flush'); },
            active => calls.push(active ? 'active' : 'inactive'),
        );

        await withCanvasPathMigrationBarrier(async () => { calls.push('operation'); });
        expect(calls).toEqual(['active', 'flush', 'operation', 'inactive']);

        unregister();
        await withCanvasPathMigrationBarrier(async () => { calls.push('unregistered'); });
        expect(calls.at(-1)).toBe('unregistered');
    });

    it('fails closed and never starts migration when the strict canvas save fails', async () => {
        const operation = vi.fn(async () => undefined);
        const active: boolean[] = [];

        const unregister = registerCanvasPathMigrationParticipant(
            async () => { throw new Error('save failed'); },
            value => active.push(value),
        );

        await expect(withCanvasPathMigrationBarrier(operation)).rejects.toThrow('save failed');

        expect(operation).not.toHaveBeenCalled();
        expect(active).toEqual([true, false]);
        unregister();
    });

    it('serializes overlapping migrations so their barriers never overlap', async () => {
        let releaseFirst!: () => void;
        const firstGate = new Promise<void>(resolve => { releaseFirst = resolve; });
        const calls: string[] = [];

        const first = withCanvasPathMigrationBarrier(async () => {
            calls.push('first-start');
            await firstGate;
            calls.push('first-end');
        });
        const second = withCanvasPathMigrationBarrier(async () => {
            calls.push('second-start');
            calls.push('second-end');
        });
        await vi.waitFor(() => expect(calls).toEqual(['first-start']));

        releaseFirst();
        await Promise.all([first, second]);
        expect(calls).toEqual(['first-start', 'first-end', 'second-start', 'second-end']);
    });

    it('waits for retiring canvases even after a replacement registers', async () => {
        let releaseOld!: () => void;
        const oldSave = new Promise<void>(resolve => { releaseOld = resolve; });
        const calls: string[] = [];
        const unregisterOld = registerCanvasPathMigrationParticipant(
            async () => { calls.push('old-flush'); await oldSave; },
            active => calls.push(`old-${active}`),
        );
        const unregisterNew = registerCanvasPathMigrationParticipant(
            async () => { calls.push('new-flush'); },
            active => calls.push(`new-${active}`),
        );

        const migration = withCanvasPathMigrationBarrier(async () => { calls.push('operation'); });
        await vi.waitFor(() => {
            expect(calls).toContain('old-flush');
            expect(calls).toContain('new-flush');
        });
        expect(calls).not.toContain('operation');
        releaseOld();
        await migration;
        expect(calls.indexOf('operation')).toBeGreaterThan(calls.indexOf('old-flush'));
        expect(calls.indexOf('operation')).toBeGreaterThan(calls.indexOf('new-flush'));
        unregisterOld();
        unregisterNew();
    });

    it('makes a canvas mounted during migration inherit the active state', async () => {
        let release!: () => void;
        const gate = new Promise<void>(resolve => { release = resolve; });
        let operationStarted = false;
        const migration = withCanvasPathMigrationBarrier(() => {
            operationStarted = true;
            return gate;
        });
        await vi.waitFor(() => expect(operationStarted).toBe(true));
        const active: boolean[] = [];
        const unregister = registerCanvasPathMigrationParticipant(async () => undefined, value => active.push(value));

        expect(active).toEqual([true]);
        release();
        await migration;
        expect(active).toEqual([true, false]);
        unregister();
    });
});
