interface CanvasPathMigrationParticipant {
    flushStrict: () => Promise<void>;
    setActive: (active: boolean) => void;
}

const participants = new Set<CanvasPathMigrationParticipant>();
let migrationQueue: Promise<void> = Promise.resolve();
let migrationActive = false;

export function registerCanvasPathMigrationParticipant(
    flushStrict: () => Promise<void>,
    setActive: (active: boolean) => void,
): () => void {
    const participant = { flushStrict, setActive };
    participants.add(participant);
    if (migrationActive) setActive(true);
    return () => {
        participants.delete(participant);
        setActive(false);
    };
}

export async function withCanvasPathMigrationBarrier<T>(operation: () => Promise<T>): Promise<T> {
    const run = async () => {
        migrationActive = true;
        participants.forEach(participant => participant.setActive(true));
        try {
            await Promise.all(
                Array.from(participants, participant => participant.flushStrict()),
            );
            return await operation();
        } finally {
            migrationActive = false;
            participants.forEach(participant => participant.setActive(false));
        }
    };
    const queued = migrationQueue.then(run, run);
    migrationQueue = queued.then(() => undefined, () => undefined);
    return queued;
}
