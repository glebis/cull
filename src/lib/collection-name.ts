export interface ImportCollectionNameItem {
    path: string;
    aiPrompt?: string | null;
    generationPrompt?: string | null;
    importedAt?: string | null;
}

export interface ImportCollectionNameOptions {
    now?: Date;
    maxLength?: number;
}

const GENERIC_PATH_WORDS = new Set([
    'asset',
    'assets',
    'brain',
    'brains',
    'desktop',
    'document',
    'documents',
    'download',
    'downloads',
    'export',
    'exports',
    'final',
    'generated',
    'generation',
    'generations',
    'home',
    'image',
    'images',
    'img',
    'imgs',
    'input',
    'inputs',
    'new',
    'output',
    'outputs',
    'photo',
    'photos',
    'picture',
    'pictures',
    'temp',
    'tmp',
    'upscale',
    'upscaled',
    'user',
    'users',
]);

const GENERIC_FILE_WORDS = new Set([
    ...GENERIC_PATH_WORDS,
    'copy',
    'edit',
    'file',
    'frame',
    'generated',
    'image',
    'img',
    'screenshot',
    'untitled',
    'version',
]);

const PROMPT_NOISE = new Set([
    '4k',
    '8k',
    'award winning',
    'best quality',
    'cinematic lighting',
    'digital art',
    'high quality',
    'highly detailed',
    'masterpiece',
    'octane render',
    'photorealistic',
    'trending on artstation',
    'ultra detailed',
    'unreal engine',
]);

const SMALL_TITLE_WORDS = new Set(['a', 'an', 'and', 'as', 'at', 'by', 'for', 'from', 'in', 'of', 'on', 'or', 'the', 'to', 'with']);

const BRAND_WORDS: Record<string, string> = {
    'ai': 'AI',
    'api': 'API',
    'dall e': 'DALL-E',
    'dalle': 'DALL-E',
    'gemini': 'Gemini',
    'gpt': 'GPT',
    'jpeg': 'JPEG',
    'jpg': 'JPG',
    'midjourney': 'Midjourney',
    'mj': 'MJ',
    'openai': 'OpenAI',
    'png': 'PNG',
    'sd': 'SD',
    'sdxl': 'SDXL',
    'stable diffusion': 'Stable Diffusion',
};

export function formatCollectionDate(date = new Date()): string {
    const year = date.getFullYear();
    const month = pad2(date.getMonth() + 1);
    const day = pad2(date.getDate());
    const hour = pad2(date.getHours());
    const minute = pad2(date.getMinutes());
    return `${year}.${month}.${day} ${hour}:${minute}`;
}

export function generateImportCollectionName(
    items: ImportCollectionNameItem[],
    options: ImportCollectionNameOptions = {}
): string {
    const timestamp = formatCollectionDate(options.now ?? firstImportedAt(items) ?? new Date());
    const maxLength = options.maxLength ?? 86;
    const promptTitle = bestPromptTitle(items);
    const folderTitle = bestFolderTitle(items);
    const fileTitle = bestFileTitle(items);

    let base = promptTitle;
    if (!base) {
        const parts = [folderTitle, fileTitle]
            .filter((part): part is string => Boolean(part))
            .filter((part, index, all) => all.findIndex(other => normalizedKey(other) === normalizedKey(part)) === index);
        base = parts.join(' - ') || 'Import';
    }

    return trimCollectionName(`${base} - ${timestamp}`, maxLength);
}

function firstImportedAt(items: ImportCollectionNameItem[]): Date | null {
    const dates = items
        .map(item => item.importedAt)
        .filter((value): value is string => Boolean(value))
        .map(value => new Date(value))
        .filter(date => Number.isFinite(date.getTime()))
        .sort((a, b) => a.getTime() - b.getTime());

    return dates[0] ?? null;
}

function bestPromptTitle(items: ImportCollectionNameItem[]): string | null {
    const titles = items
        .map(item => item.generationPrompt || item.aiPrompt)
        .filter((prompt): prompt is string => Boolean(prompt?.trim()))
        .map(extractPromptTitle)
        .filter((title): title is string => Boolean(title));

    if (titles.length === 0) return null;

    const counts = new Map<string, { title: string; count: number }>();
    for (const title of titles) {
        const key = normalizedKey(title);
        const existing = counts.get(key);
        counts.set(key, { title, count: (existing?.count ?? 0) + 1 });
    }

    const ranked = [...counts.values()].sort((a, b) => b.count - a.count);
    if (ranked[0]?.count > 1 || ranked.length === 1) {
        return ranked[0].title;
    }

    const shared = sharedTitleWords(titles);
    if (shared) return shared;

    return `${ranked[0].title} Set`;
}

function extractPromptTitle(prompt: string): string | null {
    let text = prompt
        .replace(/\s+/g, ' ')
        .replace(/^\/imagine\s+prompt:\s*/i, '')
        .replace(/^prompt:\s*/i, '')
        .trim();

    const midjourneyArgs = text.search(/\s--[a-z0-9-]+/i);
    if (midjourneyArgs >= 0) {
        text = text.slice(0, midjourneyArgs).trim();
    }

    const parts = text
        .split(/[.;,\n]/)
        .map(part => cleanWords(part))
        .filter(part => part.length > 0)
        .filter(part => !PROMPT_NOISE.has(part.toLowerCase()));

    if (parts.length === 0) return null;

    const words = parts
        .slice(0, 2)
        .join(' ')
        .split(/\s+/)
        .filter(Boolean)
        .slice(0, 9);

    if (words.length === 0) return null;
    return titleCase(words.join(' '));
}

function bestFolderTitle(items: ImportCollectionNameItem[]): string | null {
    const directories = items
        .map(item => pathSegments(item.path).slice(0, -1))
        .filter(segments => segments.length > 0);

    if (directories.length === 0) return null;

    const common = commonPrefix(directories);
    const commonTitle = titleFromPathSegments(common);
    if (commonTitle) return commonTitle;

    const parentTitles = directories
        .map(segments => titleFromPathSegments(segments.slice(-2)))
        .filter((title): title is string => Boolean(title));
    return mostFrequent(parentTitles);
}

function bestFileTitle(items: ImportCollectionNameItem[]): string | null {
    const stems = items
        .map(item => fileStem(item.path))
        .map(stem => stem.trim())
        .filter(Boolean);

    if (stems.length === 0) return null;

    if (stems.length === 1) {
        return titleFromSegment(stems[0], GENERIC_FILE_WORDS);
    }

    const prefix = commonStemPrefix(stems);
    const prefixTitle = titleFromSegment(prefix, GENERIC_FILE_WORDS);
    if (prefixTitle && tokenCount(prefixTitle) >= 2) return prefixTitle;

    const tokenized = stems.map(stem => tokensFromSegment(stem, GENERIC_FILE_WORDS));
    const threshold = Math.max(2, Math.ceil(stems.length * 0.45));
    const counts = new Map<string, number>();
    for (const tokens of tokenized) {
        for (const token of new Set(tokens)) {
            counts.set(token, (counts.get(token) ?? 0) + 1);
        }
    }

    const firstOrder = tokenized[0] ?? [];
    const shared = firstOrder
        .filter(token => (counts.get(token) ?? 0) >= threshold)
        .slice(0, 6);

    if (shared.length >= 2) return titleCase(shared.join(' '));
    return null;
}

function titleFromPathSegments(segments: string[]): string | null {
    const candidates = [...segments].reverse();
    for (let i = 0; i < candidates.length; i += 1) {
        const title = titleFromSegment(candidates[i], GENERIC_PATH_WORDS);
        if (!title) continue;

        if (isDateOnlyTitle(title) && i + 1 < candidates.length) {
            const parent = titleFromSegment(candidates[i + 1], GENERIC_PATH_WORDS);
            return parent ? `${parent} ${title}` : title;
        }

        return title;
    }
    return null;
}

function titleFromSegment(segment: string, genericWords: Set<string>): string | null {
    const tokens = tokensFromSegment(segment, genericWords);
    if (tokens.length === 0) return null;
    return titleCase(tokens.slice(0, 8).join(' '));
}

function tokensFromSegment(segment: string, genericWords: Set<string>): string[] {
    const normalized = normalizeDateTokens(splitCamelCase(decodePathPart(segment)))
        .replace(/\.[a-z0-9]{2,5}$/i, '')
        .replace(/[()[\]{}]/g, ' ')
        .replace(/['"]/g, '')
        .replace(/[_\-+]+/g, ' ')
        .replace(/[^\p{L}\p{N}.:]+/gu, ' ')
        .replace(/\s+/g, ' ')
        .trim()
        .toLowerCase();

    if (!normalized) return [];

    return normalized
        .split(/\s+/)
        .map(token => token.replace(/^[.:]+|[.:]+$/g, ''))
        .filter(Boolean)
        .filter(token => !genericWords.has(token))
        .filter(token => !isLowSignalToken(token));
}

function cleanWords(value: string): string {
    return value
        .replace(/[()[\]{}]/g, ' ')
        .replace(/['"]/g, '')
        .replace(/\s+/g, ' ')
        .trim();
}

function sharedTitleWords(titles: string[]): string | null {
    if (titles.length < 2) return null;
    const first = titleTokens(titles[0]);
    const titleSets = titles.map(title => new Set(titleTokens(title)));
    const threshold = Math.ceil(titles.length * 0.6);
    const shared = first
        .filter(token => !SMALL_TITLE_WORDS.has(token))
        .filter(token => titleSets.filter(set => set.has(token)).length >= threshold)
        .slice(0, 6);

    if (shared.length < 2) return null;
    return titleCase(shared.join(' '));
}

function titleTokens(title: string): string[] {
    return title.toLowerCase().split(/\s+/).filter(Boolean);
}

function commonPrefix(paths: string[][]): string[] {
    if (paths.length === 0) return [];
    const [first, ...rest] = paths;
    const result: string[] = [];

    for (let i = 0; i < first.length; i += 1) {
        if (rest.every(path => path[i] === first[i])) {
            result.push(first[i]);
        } else {
            break;
        }
    }

    return result;
}

function commonStemPrefix(stems: string[]): string {
    if (stems.length === 0) return '';
    let prefix = stems[0];
    for (const stem of stems.slice(1)) {
        while (prefix && !stem.startsWith(prefix)) {
            prefix = prefix.slice(0, -1);
        }
    }
    return prefix.replace(/[_\-\s.]+$/, '');
}

function mostFrequent(values: string[]): string | null {
    if (values.length === 0) return null;
    const counts = new Map<string, { value: string; count: number }>();
    for (const value of values) {
        const key = normalizedKey(value);
        const existing = counts.get(key);
        counts.set(key, { value, count: (existing?.count ?? 0) + 1 });
    }

    const [winner] = [...counts.values()].sort((a, b) => b.count - a.count);
    return winner?.value ?? null;
}

function pathSegments(path: string): string[] {
    return path
        .replace(/\\/g, '/')
        .split('/')
        .map(part => part.trim())
        .filter(Boolean);
}

function fileStem(path: string): string {
    const file = pathSegments(path).at(-1) ?? path;
    return file.replace(/\.[^.]+$/, '');
}

function decodePathPart(value: string): string {
    try {
        return decodeURIComponent(value);
    } catch {
        return value;
    }
}

function splitCamelCase(value: string): string {
    return value
        .replace(/([a-z])([A-Z])/g, '$1 $2')
        .replace(/([A-Z]+)([A-Z][a-z])/g, '$1 $2');
}

function normalizeDateTokens(value: string): string {
    return value
        .replace(/\b(20\d{2})[-_./ ]?(0[1-9]|1[0-2])[-_./ ]?([0-2]\d|3[01])\b/g, '$1.$2.$3')
        .replace(/\b(20\d{2})[-_./ ]([1-9])[-_./ ]([0-2]?\d|3[01])\b/g, (_, y, m, d) => `${y}.${pad2(Number(m))}.${pad2(Number(d))}`);
}

function isLowSignalToken(token: string): boolean {
    if (/^[a-z]$/i.test(token)) return true;
    if (/^\d+$/.test(token)) return true;
    if (/^\d{1,2}:\d{2}(:\d{2})?$/.test(token)) return true;
    if (/^20\d{2}\.\d{2}\.\d{2}$/.test(token)) return false;
    if (/^[a-f0-9]{8,}$/i.test(token)) return true;
    if (/^\d{3,}[a-z]?$/i.test(token)) return true;
    if (/^\d+x\d+$/i.test(token)) return true;
    return false;
}

function isDateOnlyTitle(title: string): boolean {
    return /^20\d{2}\.\d{2}\.\d{2}$/.test(title);
}

function titleCase(value: string): string {
    const normalized = value.replace(/\s+/g, ' ').trim().toLowerCase();
    const brandNormalized = BRAND_WORDS[normalized];
    if (brandNormalized) return brandNormalized;

    return normalized
        .split(' ')
        .map((word, index, words) => {
            const brand = BRAND_WORDS[word];
            if (brand) return brand;
            if (index > 0 && index < words.length - 1 && SMALL_TITLE_WORDS.has(word)) return word;
            return word.charAt(0).toUpperCase() + word.slice(1);
        })
        .join(' ')
        .replace(/\bStable Diffusion\b/gi, 'Stable Diffusion')
        .replace(/\bDall E\b/gi, 'DALL-E');
}

function normalizedKey(value: string): string {
    return value.toLowerCase().replace(/[^a-z0-9]+/g, ' ').trim();
}

function tokenCount(value: string): number {
    return value.split(/\s+/).filter(Boolean).length;
}

function trimCollectionName(value: string, maxLength: number): string {
    if (value.length <= maxLength) return value;
    const timestampLength = ' - YYYY.MM.DD HH:mm'.length;
    const baseMax = Math.max(18, maxLength - timestampLength);
    const [base, timestamp] = value.split(/ - (?=20\d{2}\.\d{2}\.\d{2} \d{2}:\d{2}$)/);
    if (!timestamp) return `${value.slice(0, maxLength - 1).trim()}...`;
    return `${base.slice(0, baseMax).replace(/\s+\S*$/, '').trim()} - ${timestamp}`;
}

function pad2(value: number): string {
    return value.toString().padStart(2, '0');
}
