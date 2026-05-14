import { UMAP } from 'umap-js';

interface ProjectionImageMeta {
    id: string;
    path: string;
    thumbnailPath: string | null;
}

interface ProjectionRequest {
    requestId: number;
    provider: 'clip' | 'gemini';
    embeddings: [string, number[]][];
    images: ProjectionImageMeta[];
}

interface Point {
    id: string;
    x: number;
    y: number;
    cluster: number;
}

interface Cluster {
    id: number;
    label: string;
    count: number;
    colorIndex: number;
    previewPaths: string[];
    x: number;
    y: number;
}

function seededRandom(seed: number) {
    let state = seed >>> 0;
    return () => {
        state = (state * 1664525 + 1013904223) >>> 0;
        return state / 0x100000000;
    };
}

function hashString(input: string): number {
    let hash = 0x811c9dc5;
    for (let i = 0; i < input.length; i++) {
        hash ^= input.charCodeAt(i);
        hash = Math.imul(hash, 0x01000193);
    }
    return hash >>> 0;
}

function projectionKey(provider: 'clip' | 'gemini', ids: string[]): string {
    const sorted = [...ids].sort();
    let hash = 0x811c9dc5;
    for (const id of sorted) {
        hash ^= hashString(id);
        hash = Math.imul(hash, 0x01000193);
    }
    return `${provider}:${sorted.length}:${(hash >>> 0).toString(16).padStart(8, '0')}`;
}

function kMeans(data: number[][], k: number): number[] {
    if (data.length === 0) return [];

    const centroids: number[][] = [];
    for (let i = 0; i < Math.min(k, data.length); i++) {
        const idx = Math.floor((i / Math.max(1, k - 1)) * (data.length - 1));
        centroids.push([...data[idx]]);
    }

    const labels = new Array(data.length).fill(0);
    for (let iter = 0; iter < 20; iter++) {
        for (let i = 0; i < data.length; i++) {
            let minDist = Infinity;
            for (let c = 0; c < centroids.length; c++) {
                const dx = data[i][0] - centroids[c][0];
                const dy = data[i][1] - centroids[c][1];
                const dist = dx * dx + dy * dy;
                if (dist < minDist) {
                    minDist = dist;
                    labels[i] = c;
                }
            }
        }

        const sums = centroids.map(() => [0, 0]);
        const counts = new Array(centroids.length).fill(0);
        for (let i = 0; i < data.length; i++) {
            sums[labels[i]][0] += data[i][0];
            sums[labels[i]][1] += data[i][1];
            counts[labels[i]]++;
        }
        for (let c = 0; c < centroids.length; c++) {
            if (counts[c] > 0) {
                centroids[c] = [sums[c][0] / counts[c], sums[c][1] / counts[c]];
            }
        }
    }

    return labels;
}

function folderName(path: string): string {
    const parts = path.split('/');
    return parts.length >= 2 ? parts[parts.length - 2] : 'unknown';
}

self.onmessage = (event: MessageEvent<ProjectionRequest>) => {
    const { requestId, provider, embeddings, images } = event.data;
    const imageMap = new Map(images.map(img => [img.id, img]));
    const ids = embeddings.map(([id]) => id);
    const vectors = embeddings.map(([, vector]) => vector);

    if (vectors.length < 2) {
        self.postMessage({
            requestId,
            points: [],
            clusters: [],
            projectionKey: projectionKey(provider, ids),
        });
        return;
    }

    const seed = hashString(projectionKey(provider, ids));
    const nNeighbors = Math.min(15, Math.max(2, Math.floor(vectors.length / 5)));
    const umap = new UMAP({
        random: seededRandom(seed),
        nNeighbors,
        minDist: 0.05,
        spread: 1.5,
        nComponents: 2,
    });
    const projection = umap.fit(vectors);
    const k = Math.min(16, Math.max(3, Math.floor(Math.sqrt(projection.length))));
    const clusterLabels = kMeans(projection, k);

    const points: Point[] = embeddings.map(([id], i) => ({
        id,
        x: projection[i][0],
        y: projection[i][1],
        cluster: clusterLabels[i],
    }));

    const clusterGroups = new Map<number, Point[]>();
    for (const point of points) {
        if (!clusterGroups.has(point.cluster)) clusterGroups.set(point.cluster, []);
        clusterGroups.get(point.cluster)!.push(point);
    }

    const clusters: Cluster[] = Array.from(clusterGroups.entries())
        .sort((a, b) => b[1].length - a[1].length)
        .map(([clusterId, clusterPoints]) => {
            const folderCounts = new Map<string, number>();
            const previewPaths: string[] = [];
            let x = 0;
            let y = 0;

            for (const point of clusterPoints) {
                x += point.x;
                y += point.y;
                const img = imageMap.get(point.id);
                if (!img) continue;
                const folder = folderName(img.path);
                folderCounts.set(folder, (folderCounts.get(folder) ?? 0) + 1);
                if (previewPaths.length < 4 && img.thumbnailPath) {
                    previewPaths.push(img.thumbnailPath);
                }
            }

            let label = 'cluster';
            let bestCount = 0;
            for (const [name, count] of folderCounts) {
                if (count > bestCount) {
                    label = name;
                    bestCount = count;
                }
            }

            return {
                id: clusterId,
                label,
                count: clusterPoints.length,
                colorIndex: clusterId,
                previewPaths,
                x: x / clusterPoints.length,
                y: y / clusterPoints.length,
            };
        });

    self.postMessage({
        requestId,
        points,
        clusters,
        projectionKey: projectionKey(provider, ids),
    });
};
