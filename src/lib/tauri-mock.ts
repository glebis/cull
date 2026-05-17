const MOCK_SMART_COLLECTIONS = [
  { id: 'preset-1', name: '5 Stars', description: null, collection_type: 'smart', filter_json: '{"type":"rule","field":"rating","op":"eq","value":5.0}', nl_query: null, is_preset: true, sort_order: 1, created_at: '2026-01-01', image_count: 124 },
  { id: 'preset-2', name: '4 Stars+', description: null, collection_type: 'smart', filter_json: '{"type":"rule","field":"rating","op":"gte","value":4.0}', nl_query: null, is_preset: true, sort_order: 2, created_at: '2026-01-01', image_count: 389 },
  { id: 'preset-3', name: 'Picks', description: null, collection_type: 'smart', filter_json: '{"type":"rule","field":"decision","op":"eq","value":"accept"}', nl_query: null, is_preset: true, sort_order: 3, created_at: '2026-01-01', image_count: 67 },
  { id: 'preset-4', name: 'Landscape', description: null, collection_type: 'smart', filter_json: '{"type":"rule","field":"orientation","op":"eq","value":"landscape"}', nl_query: null, is_preset: true, sort_order: 10, created_at: '2026-01-01', image_count: 891 },
  { id: 'preset-5', name: 'Recent Imports', description: null, collection_type: 'smart', filter_json: '{"type":"rule","field":"imported_at","op":"last_n_days","value":7.0}', nl_query: null, is_preset: true, sort_order: 6, created_at: '2026-01-01', image_count: 42 },
];

let userCollections: typeof MOCK_SMART_COLLECTIONS = [];
let nextId = 100;

const mockApiKeys: Record<string, string> = {};

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
      nl_query: args.nlQuery ?? null as any,
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

  list_export_presets: () => [
    { id: 'ig_carousel', platform: 'Instagram', format: 'Carousel', width: 1080, height: 1080, mime: 'image/png' },
    { id: 'ig_story', platform: 'Instagram', format: 'Story', width: 1080, height: 1920, mime: 'image/png' },
    { id: 'twitter_post', platform: 'Twitter', format: 'Post', width: 1200, height: 675, mime: 'image/png' },
    { id: 'a4_pdf', platform: 'Print', format: 'A4 PDF', width: 2480, height: 3508, mime: 'application/pdf' },
  ],

  create_export_manifest: (_: any, args: { imageIds: string[]; targetPresets: string[]; template?: string }) => {
    const presetMap: Record<string, any> = {
      ig_carousel: { id: 'ig_carousel', platform: 'Instagram', format: 'Carousel', width: 1080, height: 1080, mime: 'image/png' },
      ig_story: { id: 'ig_story', platform: 'Instagram', format: 'Story', width: 1080, height: 1920, mime: 'image/png' },
      twitter_post: { id: 'twitter_post', platform: 'Twitter', format: 'Post', width: 1200, height: 675, mime: 'image/png' },
      a4_pdf: { id: 'a4_pdf', platform: 'Print', format: 'A4 PDF', width: 2480, height: 3508, mime: 'application/pdf' },
    };
    const presetId = args.targetPresets[0] ?? 'ig_carousel';
    const target = presetMap[presetId] ?? presetMap.ig_carousel;
    const tmpl = args.template ?? 'bleed';
    return {
      kind: 'export_manifest',
      schema_version: 1,
      id: `manifest-${Date.now()}`,
      title: 'Mock Export',
      locale: 'en',
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
      source: { app: 'cull', collection_id: null, image_ids: args.imageIds },
      defaults: {
        template: tmpl,
        fonts: { serif: 'Georgia', mono: 'JetBrains Mono' },
        colors: { preset: 'dark', background: '#08080c', foreground: '#e0e0e0', accent: '#7aa2f7' },
        safe_area: { top: 40, right: 40, bottom: 40, left: 40 },
      },
      targets: [target],
      slides: args.imageIds.map((id: string, i: number) => ({
        id: `slide-${i}`,
        template: tmpl,
        image: { asset_id: `asset-${id}`, fit: 'cover' as const },
        text: { headline: '', body: '', caption: `Image ${i + 1}` },
        overlay: { position: 'bottom', scrim: { type: 'linear', direction: 'to top', from: 'rgba(0,0,0,0.6)', to: 'transparent' }, text_color: '#ffffff' },
        metadata: { tags: [], alt: `Image ${i + 1}` },
      })),
      assets: args.imageIds.map((id: string) => ({
        id: `asset-${id}`,
        kind: 'source' as const,
        uri: `cull://image/${id}`,
        mime: 'image/png',
        width: 1920,
        height: 1080,
      })),
      agent_tasks: [],
      agent_hints: { tone: 'neutral', allow_generated_images: false, language: 'en' },
      agent_contract: { mutable_paths: [], append_only: [], immutable_paths: [] },
    };
  },

  get_export_asset: (_: any, args: { uri: string }) => {
    const idMatch = args.uri.match(/cull:\/\/image\/(.+)/);
    const id = idMatch?.[1] ?? 'unknown';
    return { path: `/mock/export-${id}.png`, mime: 'image/png', width: 1920, height: 1080 };
  },

  save_export_image: () => '/mock/exported-slide.png',
  assemble_export_pdf: () => '/mock/exported.pdf',

  check_library_health: () => ({ purged: 0, missing_sources: 0, to_regenerate: [] }),
  regenerate_thumbnails_by_ids: () => 0,
  regenerate_single_thumbnail: () => '',
  set_api_key: (_: any, args: { provider: string; key: string }) => {
    mockApiKeys[`api_key_${args.provider}`] = args.key;
  },
  has_api_key: (_: any, args: { provider: string }) => {
    const k = mockApiKeys[`api_key_${args.provider}`];
    return k !== undefined && k !== '';
  },
  delete_api_key: (_: any, args: { provider: string }) => {
    delete mockApiKeys[`api_key_${args.provider}`];
  },
  validate_api_key: () => true,
  backfill_image_metadata: () => 0,
  backfill_image_tags: () => ({
    images_processed: 20,
    tags_created: 8,
    image_tags_created: 36,
  }),
  list_image_tags: (_: any, args: { imageId: string }) => [
    {
      id: 'tag-mock-1',
      image_id: args.imageId,
      name: 'golden hour',
      normalized_name: 'golden-hour',
      tag_type: 'vision',
      source: 'metadata:minicpm:tags',
      confidence: null,
      created_at: '2026-01-01T00:00:00Z',
    },
    {
      id: 'tag-mock-2',
      image_id: args.imageId,
      name: 'person',
      normalized_name: 'person',
      tag_type: 'object',
      source: 'detection:yolo11m',
      confidence: 0.91,
      created_at: '2026-01-01T00:00:00Z',
    },
  ],
  list_tags: () => [
    {
      id: 'tag-mock-1',
      name: 'golden hour',
      normalized_name: 'golden-hour',
      tag_type: 'vision',
      image_count: 7,
    },
    {
      id: 'tag-mock-2',
      name: 'person',
      normalized_name: 'person',
      tag_type: 'object',
      image_count: 5,
    },
  ],
  list_images: () => Array.from({ length: 20 }, (_, i) => makeMockImage(i)),
  get_image_count: () => 20,
  list_jobs: () => [],
  get_job: () => null,
  cancel_job: () => undefined,
  pause_job: () => undefined,
  resume_job: () => undefined,
  analyze_image_quality: (_: any, args: { imageIds: string[] }) => args.imageIds.length,
  get_image_quality: (_: any, args: { imageId: string }) => ({
    image_id: args.imageId,
    analyzer_version: 'quality-v1',
    focus_score: 125,
    blur_score: 0.44,
    exposure_score: 0.88,
    clipped_shadow_pct: 0.01,
    clipped_highlight_pct: 0.02,
    mean_luma: 0.52,
    contrast: 0.31,
    analyzed_at: '2026-01-01T00:00:00Z',
  }),
  get_quality_count: () => 12,
  analyze_image_colors: (_: any, args: { imageIds: string[] }) => args.imageIds.length,
  get_image_color_metrics: (_: any, args: { imageId: string }) => ({
    image_id: args.imageId,
    analyzer_version: 'color-v1',
    dominant_hex: '#7aa2f7',
    palette: [
      { hex: '#7aa2f7', red: 122, green: 162, blue: 247, percentage: 0.46 },
      { hex: '#0c0c12', red: 12, green: 12, blue: 18, percentage: 0.32 },
      { hex: '#e0af68', red: 224, green: 175, blue: 104, percentage: 0.22 },
    ],
    dominant_hue_bucket: 'blue',
    mean_luma: 0.42,
    mean_saturation: 0.38,
    colorfulness: 0.51,
    contrast: 0.29,
    analyzed_at: '2026-01-01T00:00:00Z',
  }),
  get_color_metrics_count: () => 18,
  list_images_by_color_bucket: () => [makeMockImage(1), makeMockImage(4), makeMockImage(7)],
  analyze_perceptual_hashes: (_: any, args: { imageIds: string[] }) => args.imageIds.length,
  get_image_perceptual_hash: (_: any, args: { imageId: string }) => ({
    image_id: args.imageId,
    algorithm: 'phash-dct-64-v1',
    hash_hi: 0,
    hash_lo: 9223372036854775807,
    band0: 32767,
    band1: 65535,
    band2: 32767,
    band3: 65535,
    analyzed_at: '2026-01-01T00:00:00Z',
  }),
  get_perceptual_hash_count: () => 18,
  find_near_duplicates_by_phash: () => [
    {
      image: makeMockImage(1),
      algorithm: 'phash-dct-64-v1',
      distance: 3,
    },
  ],
  get_clip_model_download_info: () => ({
    model_id: 'clip-vit-b32',
    url: 'https://huggingface.co/Qdrant/clip-ViT-B-32-vision/resolve/main/model.onnx',
    model_path: '/mock/app-data/models/clip-vit-b32-vision.onnx',
    part_path: '/mock/app-data/models/clip-vit-b32-vision.onnx.part',
    curl_command: "mkdir -p '/mock/app-data/models' && curl -L -C - -o '/mock/app-data/models/clip-vit-b32-vision.onnx.part' 'https://huggingface.co/Qdrant/clip-ViT-B-32-vision/resolve/main/model.onnx' && mv '/mock/app-data/models/clip-vit-b32-vision.onnx.part' '/mock/app-data/models/clip-vit-b32-vision.onnx'",
  }),
  get_embedding_model_download_info: (_: any, args: { model: string }) => ({
    model_id: args.model,
    url: args.model === 'dinov2-vits14'
      ? 'https://huggingface.co/sefaburak/dinov2-small-onnx/resolve/main/dinov2_vits14.onnx'
      : 'https://huggingface.co/Qdrant/clip-ViT-B-32-vision/resolve/main/model.onnx',
    model_path: `/mock/app-data/models/${args.model}.onnx`,
    part_path: `/mock/app-data/models/${args.model}.onnx.part`,
    curl_command: `curl -L -C - -o '/mock/app-data/models/${args.model}.onnx.part'`,
  }),
  download_clip_model: () => 'already_downloaded',
  download_embedding_model: () => 'already_downloaded',
  is_model_available: () => true,
  is_embedding_model_available: () => true,
  generate_embeddings: (_: any, args: { imageIds: string[] }) => args.imageIds.length,
  generate_model_embeddings: (_: any, args: { imageIds: string[] }) => args.imageIds.length,
  generate_similarity_groups: () => ({
    model_name: 'clip-vit-b32',
    threshold: 0.88,
    method: 'greedy_threshold_v1',
    groups_created: 3,
    images_grouped: 9,
    singleton_images: 11,
  }),
  list_similarity_groups: () => [
    {
      id: 'sg-mock-1',
      model_name: 'clip-vit-b32',
      threshold: 0.88,
      method: 'greedy_threshold_v1',
      representative_image_id: 'img-1',
      image_count: 4,
      created_at: '2026-01-01T00:00:00Z',
      updated_at: '2026-01-01T00:00:00Z',
    },
  ],
  list_similarity_group_images: () => Array.from({ length: 4 }, (_, i) => makeMockImage(i)),
  list_folders: () => [],
  list_collections: () => [],
  list_images_by_folder: () => [],
  list_images_filtered: () => [],
  list_collection_images: () => [],
};

export async function invoke<T>(cmd: string, args?: any): Promise<T> {
  const handler = MOCK_HANDLERS[cmd];
  if (handler) {
    await new Promise(r => setTimeout(r, 50 + Math.random() * 100));
    return handler(null, args) as T;
  }

  console.warn(`[mock] No handler for command: ${cmd}`);
  return undefined as T;
}
