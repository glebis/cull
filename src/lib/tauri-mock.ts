function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI__' in window;
}

const MOCK_SMART_COLLECTIONS = [
  { id: 'preset-1', name: '5 Stars', description: null, collection_type: 'smart', filter_json: '{"type":"rule","field":"rating","op":"eq","value":5.0}', nl_query: null, is_preset: true, sort_order: 1, created_at: '2026-01-01', image_count: 124 },
  { id: 'preset-2', name: '4 Stars+', description: null, collection_type: 'smart', filter_json: '{"type":"rule","field":"rating","op":"gte","value":4.0}', nl_query: null, is_preset: true, sort_order: 2, created_at: '2026-01-01', image_count: 389 },
  { id: 'preset-3', name: 'Picks', description: null, collection_type: 'smart', filter_json: '{"type":"rule","field":"decision","op":"eq","value":"accept"}', nl_query: null, is_preset: true, sort_order: 3, created_at: '2026-01-01', image_count: 67 },
  { id: 'preset-4', name: 'Landscape', description: null, collection_type: 'smart', filter_json: '{"type":"rule","field":"orientation","op":"eq","value":"landscape"}', nl_query: null, is_preset: true, sort_order: 10, created_at: '2026-01-01', image_count: 891 },
  { id: 'preset-5', name: 'Recent Imports', description: null, collection_type: 'smart', filter_json: '{"type":"rule","field":"imported_at","op":"last_n_days","value":7.0}', nl_query: null, is_preset: true, sort_order: 6, created_at: '2026-01-01', image_count: 42 },
];

let userCollections: typeof MOCK_SMART_COLLECTIONS = [];
let nextId = 100;

function makeMockImage(i: number) {
  return {
    image: {
      id: `img-${i}`,
      sha256_hash: `hash${i}`,
      width: 1920,
      height: 1080,
      format: 'png',
      file_size: 2048000,
      created_at: '2026-01-01',
      imported_at: '2026-05-01',
    },
    path: `/mock/image-${i}.png`,
    thumbnail_path: null,
    selection: i % 3 === 0 ? {
      image_id: `img-${i}`,
      project_id: null,
      star_rating: Math.min(5, Math.floor(i / 2) + 1),
      color_label: null,
      decision: i % 5 === 0 ? 'accept' : 'undecided',
    } : null,
  };
}

const MOCK_HANDLERS: Record<string, (...args: any[]) => any> = {
  list_smart_collections: () => [...MOCK_SMART_COLLECTIONS, ...userCollections],

  evaluate_smart_collection: () => {
    const count = 5 + Math.floor(Math.random() * 20);
    return Array.from({ length: count }, (_, i) => makeMockImage(i));
  },

  create_smart_collection: (_: any, args: { name: string; filterJson: string; nlQuery?: string }) => {
    const id = `user-${nextId++}`;
    userCollections.push({
      id,
      name: args.name,
      description: null,
      collection_type: 'smart',
      filter_json: args.filterJson,
      nl_query: args.nlQuery ?? null,
      is_preset: false,
      sort_order: 100 + userCollections.length,
      created_at: new Date().toISOString(),
      image_count: Math.floor(Math.random() * 200),
    });
    return id;
  },

  delete_smart_collection: (_: any, args: { id: string }) => {
    userCollections = userCollections.filter(c => c.id !== args.id);
  },

  update_smart_collection: (_: any, args: { id: string; name: string; filterJson: string }) => {
    const c = userCollections.find(c => c.id === args.id);
    if (c) {
      c.name = args.name;
      c.filter_json = args.filterJson;
    }
  },

  parse_nl_query: (_: any, args: { query: string }) => {
    const q = args.query.toLowerCase();
    const rules: any[] = [];

    if (/\d\s*stars?/.test(q)) {
      const n = parseInt(q.match(/(\d)\s*stars?/)![1]);
      const hasPlus = /or more|\+|above/.test(q);
      rules.push({ type: 'rule', field: 'rating', op: hasPlus || n >= 4 ? 'gte' : 'eq', value: n });
    }
    if (/midjourney|mj\b/.test(q)) rules.push({ type: 'rule', field: 'source_label', op: 'eq', value: 'midjourney' });
    if (/stable.?diffusion|sd\b/.test(q)) rules.push({ type: 'rule', field: 'source_label', op: 'eq', value: 'stable_diffusion' });
    if (/dall[\-·]?e|chatgpt/.test(q)) rules.push({ type: 'rule', field: 'source_label', op: 'eq', value: 'dalle' });
    if (/landscape|horizontal|wide/.test(q)) rules.push({ type: 'rule', field: 'orientation', op: 'eq', value: 'landscape' });
    if (/portrait|vertical|tall/.test(q)) rules.push({ type: 'rule', field: 'orientation', op: 'eq', value: 'portrait' });
    if (/\bpng\b/.test(q)) rules.push({ type: 'rule', field: 'format', op: 'eq', value: 'png' });
    if (/picks?|accepted/.test(q)) rules.push({ type: 'rule', field: 'decision', op: 'eq', value: 'accept' });
    if (/recent/.test(q)) rules.push({ type: 'rule', field: 'imported_at', op: 'last_n_days', value: 7 });

    if (rules.length === 0) return JSON.stringify({ type: 'group', op: 'and', children: [] });
    if (rules.length === 1) return JSON.stringify(rules[0]);
    return JSON.stringify({ type: 'group', op: 'and', children: rules });
  },

  backfill_image_metadata: () => 0,
  list_images: () => Array.from({ length: 20 }, (_, i) => makeMockImage(i)),
  get_image_count: () => 20,
  list_folders: () => [],
  list_collections: () => [],
  list_images_by_folder: () => [],
  list_images_filtered: () => [],
  list_collection_images: () => [],
};

export async function invoke<T>(cmd: string, args?: any): Promise<T> {
  if (isTauri()) {
    const { invoke: tauriInvoke } = await import('@tauri-apps/api/core');
    return tauriInvoke<T>(cmd, args);
  }

  const handler = MOCK_HANDLERS[cmd];
  if (handler) {
    await new Promise(r => setTimeout(r, 50 + Math.random() * 100));
    return handler(null, args) as T;
  }

  console.warn(`[mock] No handler for command: ${cmd}`);
  return undefined as T;
}
