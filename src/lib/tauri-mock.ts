type MockListener<T = any> = (event: { event: string; payload: T }) => void;
export type UnlistenFn = () => void;

const mockListeners = new Map<string, Map<number, MockListener>>();
let nextListenerId = 1;

function emitMockEvent(event: string, payload: any) {
  const listeners = mockListeners.get(event);
  if (!listeners) return;
  for (const handler of listeners.values()) {
    handler({ event, payload });
  }
}

if (typeof window !== 'undefined') {
  (window as any).__CULL_E2E_MOCK__ = true;
  (window as any).__CULL_E2E_EMIT__ = emitMockEvent;
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

const mockApiKeys: Record<string, string> = {};

const LONG_COMPARE_FILENAMES = [
  'ig_0b99ac4db97448df0169ee70444b788191bda40ea3858ee372.png',
  'ig_0b99ac4db97448df0169ee6fbd671081918388e8c8989ab0b4.png',
];

function useLongCompareNames(): boolean {
  if (typeof window === 'undefined') return false;
  return new URLSearchParams(window.location.search).get('longCompareNames') === '1';
}

function mockImagePath(i: number): string {
  if (useLongCompareNames() && i < LONG_COMPARE_FILENAMES.length) {
    return `/mock/${LONG_COMPARE_FILENAMES[i]}`;
  }
  return `/mock/image-${i}.png`;
}

function mockThumbnailPath(i: number): string {
  return `/Users/test/Library/Application Support/com.glebkalinin.cull/thumbnails/image-${i}.jpg`;
}

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
    path: mockImagePath(i),
    thumbnail_path: mockThumbnailPath(i),
    selection: i % 3 === 0 ? {
      image_id: `img-${i}`,
      project_id: null,
      star_rating: Math.min(5, Math.floor(i / 2) + 1),
      color_label: null,
      decision: i % 5 === 0 ? 'accept' : 'undecided',
    } : null,
  };
}

function mockImageDataUri(filePath: string): string {
  const filename = filePath.split('/').pop() || 'mock image';
  const hue = Array.from(filename).reduce((sum, char) => sum + char.charCodeAt(0), 0) % 360;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360" viewBox="0 0 640 360"><rect width="640" height="360" fill="hsl(${hue},45%,18%)"/><rect x="24" y="24" width="592" height="312" fill="none" stroke="hsl(${hue},65%,58%)" stroke-width="4"/><text x="320" y="182" text-anchor="middle" font-family="monospace" font-size="28" fill="white">${filename}</text></svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

const mockCollections: [string, string, number][] = [
  ['col-picks', 'Picks', 6],
];

let clipboardMonitorStatus = {
  running: false,
  supported: true,
  access_status: 'supported',
  collection_id: null as string | null,
  collection_name: null as string | null,
  capture_dir: '/mock/clipboard-captures',
  captured_count: 0,
  capture_existing_on_start: false,
  last_error: null as string | null,
};

let previewWebStreamStatus = {
  active: false,
  url: null as string | null,
  host: null as string | null,
  bound_host: null as string | null,
  port: null as number | null,
  remote_access: false,
};

let previewState = {
  image_id: null as string | null,
  image_ids: [] as string[],
  display_mode: 'image_only',
  layout: 'single',
  overlay: {
    showFilename: false,
    showRating: false,
    showDecision: false,
    showMetadataRail: false,
    showDimensions: false,
    showFormat: false,
    showSource: false,
    showPrompt: false,
    showTags: false,
    showHistogram: false,
    railSide: 'right',
    railWidth: 'medium',
    railTextSize: 'medium',
  },
  frozen: false,
  blanked: false,
  version: 0,
  updated_at_ms: 0,
};

const mockSessions = [
  {
    id: 'session-1',
    name: 'Smoke Session',
    source_folder: '/mock/session',
    session_folder: '/mock/session',
    image_count: 4,
    created_at: '2026-05-01T12:00:00Z',
    updated_at: '2026-05-01T12:00:00Z',
  },
];

const mockCanvases = [
  {
    id: 'canvas-1',
    session_id: 'session-1',
    name: 'Smoke Canvas',
    canvas_type: 'manual',
    layout_json: null,
    created_at: '2026-05-01T12:00:00Z',
    updated_at: '2026-05-01T12:00:00Z',
  },
];

const MOCK_HANDLERS: Record<string, (...args: any[]) => any> = {
  'plugin:event|listen': () => nextListenerId++,
  'plugin:event|unlisten': () => undefined,
  'plugin:deep-link|get_current': () => [],
  'plugin:updater|check': () => null,
  'plugin:process|restart': () => undefined,
  'plugin:dialog|open': () => null,
  'plugin:opener|open_url': () => undefined,
  'plugin:opener|open_path': () => undefined,
  'plugin:opener|reveal_item_in_dir': () => undefined,

  list_smart_collections: () => [...MOCK_SMART_COLLECTIONS, ...userCollections],

  evaluate_smart_collection: () => {
    const count = 5 + Math.floor(Math.random() * 20);
    return Array.from({ length: count }, (_, i) => makeMockImage(i));
  },

  count_smart_collection: () => 7,

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
    { id: 'web_responsive', platform: 'Web', format: 'Responsive', width: 1600, height: 1000, mime: 'image/jpeg' },
    { id: 'ig_post', platform: 'Instagram', format: 'Post', width: 1080, height: 1350, mime: 'image/png' },
    { id: 'ig_square', platform: 'Instagram', format: 'Square', width: 1080, height: 1080, mime: 'image/png' },
    { id: 'ig_carousel', platform: 'Instagram', format: 'Carousel', width: 1080, height: 1350, mime: 'image/png' },
    { id: 'ig_story', platform: 'Instagram', format: 'Story', width: 1080, height: 1920, mime: 'image/png' },
    { id: 'fb_feed', platform: 'Facebook', format: 'Feed', width: 1080, height: 1350, mime: 'image/jpeg' },
    { id: 'fb_link', platform: 'Facebook', format: 'Link Preview', width: 1200, height: 630, mime: 'image/jpeg' },
    { id: 'li_post', platform: 'LinkedIn', format: 'Post', width: 1200, height: 628, mime: 'image/jpeg' },
    { id: 'li_square', platform: 'LinkedIn', format: 'Square', width: 1200, height: 1200, mime: 'image/png' },
    { id: 'portfolio_pdf', platform: 'PDF', format: 'Portfolio', width: 2480, height: 3508, mime: 'application/pdf' },
  ],

  create_export_manifest: (_: any, args: { imageIds: string[]; targetPresets: string[]; template?: string }) => {
    const presetMap: Record<string, any> = {
      web_responsive: { id: 'web_responsive', platform: 'Web', format: 'Responsive', width: 1600, height: 1000, mime: 'image/jpeg' },
      ig_post: { id: 'ig_post', platform: 'Instagram', format: 'Post', width: 1080, height: 1350, mime: 'image/png' },
      ig_square: { id: 'ig_square', platform: 'Instagram', format: 'Square', width: 1080, height: 1080, mime: 'image/png' },
      ig_carousel: { id: 'ig_carousel', platform: 'Instagram', format: 'Carousel', width: 1080, height: 1350, mime: 'image/png' },
      ig_story: { id: 'ig_story', platform: 'Instagram', format: 'Story', width: 1080, height: 1920, mime: 'image/png' },
      fb_feed: { id: 'fb_feed', platform: 'Facebook', format: 'Feed', width: 1080, height: 1350, mime: 'image/jpeg' },
      fb_link: { id: 'fb_link', platform: 'Facebook', format: 'Link Preview', width: 1200, height: 630, mime: 'image/jpeg' },
      li_post: { id: 'li_post', platform: 'LinkedIn', format: 'Post', width: 1200, height: 628, mime: 'image/jpeg' },
      li_square: { id: 'li_square', platform: 'LinkedIn', format: 'Square', width: 1200, height: 1200, mime: 'image/png' },
      portfolio_pdf: { id: 'portfolio_pdf', platform: 'PDF', format: 'Portfolio', width: 2480, height: 3508, mime: 'application/pdf' },
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
  save_png_to_path: (_: any, args: { outputPath: string }) => args.outputPath,
  assemble_export_pdf: () => '/mock/exported.pdf',
  export_static_publish_package: () => ({
    export_dir: '/mock/static-publishing/client-review',
    site_dir: '/mock/static-publishing/client-review/site',
    manifest_path: '/mock/static-publishing/client-review/site/data/canvas.json',
    instructions_path: '/mock/static-publishing/client-review/instructions/CLAUDE.md',
    qr_svg_path: '/mock/static-publishing/client-review/site/qr.svg',
    qr_svg_data_url: 'data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIyODAiIGhlaWdodD0iMjgwIiB2aWV3Qm94PSIwIDAgMjgwIDI4MCI+PHJlY3Qgd2lkdGg9IjI4MCIgaGVpZ2h0PSIyODAiIGZpbGw9IiNmZmYiLz48ZyBmaWxsPSIjMDgwODBjIj48cmVjdCB4PSIyNCIgeT0iMjQiIHdpZHRoPSI1NiIgaGVpZ2h0PSI1NiIvPjxyZWN0IHg9IjIwMCIgeT0iMjQiIHdpZHRoPSI1NiIgaGVpZ2h0PSI1NiIvPjxyZWN0IHg9IjI0IiB5PSIyMDAiIHdpZHRoPSI1NiIgaGVpZ2h0PSI1NiIvPjxyZWN0IHg9IjEwNCIgeT0iMTA0IiB3aWR0aD0iMTYiIGhlaWdodD0iMTYiLz48cmVjdCB4PSIxMzYiIHk9IjEwNCIgd2lkdGg9IjMyIiBoZWlnaHQ9IjE2Ii8+PHJlY3QgeD0iMTg0IiB5PSIxMjAiIHdpZHRoPSIxNiIgaGVpZ2h0PSI0OCIvPjxyZWN0IHg9IjEwNCIgeT0iMTUyIiB3aWR0aD0iNDgiIGhlaWdodD0iMTYiLz48cmVjdCB4PSIxNTIiIHk9IjE4NCIgd2lkdGg9IjY0IiBoZWlnaHQ9IjE2Ii8+PHJlY3QgeD0iMjMyIiB5PSIxODQiIHdpZHRoPSIyNCIgaGVpZ2h0PSIyNCIvPjxyZWN0IHg9IjEwNCIgeT0iMjE2IiB3aWR0aD0iMTYiIGhlaWdodD0iNDAiLz48cmVjdCB4PSIxMzYiIHk9IjIzMiIgd2lkdGg9IjMyIiBoZWlnaHQ9IjI0Ii8+PC9nPjwvc3ZnPg==',
    qr_target_url: 'http://localhost:8000/',
    access_phrase: 'amber-canvas-river',
    image_count: 4,
    skipped_count: 0,
    warnings: [],
  }),
  serve_static_publish_package: () => ({
    url: 'http://127.0.0.1:8000/',
    host: '127.0.0.1',
    port: 8000,
    site_dir: '/mock/static-publishing/client-review/site',
  }),
  stop_static_publish_server: () => ({
    stopped: true,
    url: 'http://127.0.0.1:8000/',
  }),
  open_preview_display: () => 'preview-display',
  set_preview_display_always_on_top: (_: any, args: any) => args.alwaysOnTop === true,
  list_preview_display_monitors: () => [
    {
      id: 'built-in-retina-display-0x0-3024x1964',
      name: 'Built-in Retina Display',
      x: 0,
      y: 0,
      width: 3024,
      height: 1964,
      scale_factor: 2,
      primary: true,
    },
    {
      id: 'sidecar-ipad-3024x0-2732x2048',
      name: 'Sidecar iPad',
      x: 3024,
      y: 0,
      width: 2732,
      height: 2048,
      scale_factor: 2,
      primary: false,
    },
  ],
  place_preview_display: () => 'preview-display',
  get_preview_state: () => previewState,
  update_preview_state: (_: any, args: any) => {
    const imageIds = args.imageIds ?? (args.imageId ? [args.imageId] : []);
    previewState = {
      image_id: args.imageId ?? imageIds[0] ?? null,
      image_ids: imageIds,
      display_mode: args.displayMode ?? previewState.display_mode,
      layout: args.layout ?? previewState.layout,
      overlay: args.overlay ? { ...previewState.overlay, ...args.overlay } : previewState.overlay,
      frozen: args.frozen ?? previewState.frozen,
      blanked: args.blanked ?? previewState.blanked,
      version: previewState.version + 1,
      updated_at_ms: Date.now(),
    };
    emitMockEvent('preview:state-changed', previewState);
    return previewState;
  },
  get_preview_display_web_stream_status: () => previewWebStreamStatus,
  start_preview_display_web_stream: () => {
    previewWebStreamStatus = {
      active: true,
      url: 'http://127.0.0.1:8723/?token=mock-preview-token',
      host: '127.0.0.1',
      bound_host: '127.0.0.1',
      port: 8723,
      remote_access: false,
    };
    return previewWebStreamStatus;
  },
  stop_preview_display_web_stream: () => {
    previewWebStreamStatus = {
      active: false,
      url: null,
      host: null,
      bound_host: null,
      port: null,
      remote_access: false,
    };
    return previewWebStreamStatus;
  },
  get_clipboard_monitor_status: () => clipboardMonitorStatus,
  start_clipboard_monitor: () => {
    clipboardMonitorStatus = {
      ...clipboardMonitorStatus,
      running: true,
      collection_id: 'col_clipboard_mock',
      collection_name: 'Clipboard 2026.05.30 14:35',
    };
    if (!mockCollections.some(([id]) => id === 'col_clipboard_mock')) {
      mockCollections.unshift(['col_clipboard_mock', 'Clipboard 2026.05.30 14:35', 0]);
    }
    return clipboardMonitorStatus;
  },
  stop_clipboard_monitor: () => {
    clipboardMonitorStatus = { ...clipboardMonitorStatus, running: false };
    return clipboardMonitorStatus;
  },
  set_clipboard_monitor_capture_dir: (_: any, args: { path: string }) => {
    clipboardMonitorStatus = { ...clipboardMonitorStatus, capture_dir: args.path };
    return clipboardMonitorStatus;
  },
  set_clipboard_monitor_capture_existing_on_start: (_: any, args: { enabled: boolean }) => {
    clipboardMonitorStatus = { ...clipboardMonitorStatus, capture_existing_on_start: args.enabled };
    return clipboardMonitorStatus;
  },
  move_clipboard_capture_folder: (_: any, args: { newPath: string }) => {
    clipboardMonitorStatus = { ...clipboardMonitorStatus, capture_dir: args.newPath };
    return clipboardMonitorStatus;
  },
  publish_clipboard_collection: () => ({
    collection_id: clipboardMonitorStatus.collection_id ?? 'col_clipboard_mock',
    image_count: clipboardMonitorStatus.captured_count,
    site_dir: '/mock/static-publishing/clipboard/site',
    url: 'http://127.0.0.1:8000/',
    manifest_path: '/mock/static-publishing/clipboard/site/data/canvas.json',
    instructions_path: '/mock/static-publishing/clipboard/instructions/CLAUDE.md',
  }),

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
  get_app_setting: () => null,
  set_app_setting: () => undefined,
  apply_app_icon_variant: () => undefined,
  update_menu_state: () => undefined,
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
  list_image_ids: () => Array.from({ length: 20 }, (_, i) => `img-${i}`),
  get_images_by_ids: (_: any, args: { imageIds: string[] }) =>
    args.imageIds.map(id => makeMockImage(Number(id.replace('img-', '')) || 0)),
  get_image_by_path: (_: any, args: { path: string }) => ({
    ...makeMockImage(0),
    image: {
      ...makeMockImage(0).image,
      id: args.path.split('/').pop()?.replace(/\W+/g, '-') || 'transformed',
    },
    path: args.path,
  }),
  get_embedding_count: (_: any, args?: { model?: string | null }) => {
    if (args?.model === 'dinov2-vits14') return 8;
    if (args?.model === 'gemini-embedding-2') return 0;
    if (args?.model?.startsWith('cohere:')) return 7;
    if (args?.model?.startsWith('openai:')) return 6;
    if (args?.model?.startsWith('ollama:')) return 5;
    return 12;
  },
  get_embedding_page: (_: any, args?: { model?: string | null; limit?: number }) => {
    const model = args?.model ?? 'clip-vit-b32';
    const dims = model === 'dinov2-vits14' ? 384 : model.startsWith('cohere:') ? 1024 : model.startsWith('openai:') || model === 'gemini-embedding-2' ? 3072 : model.startsWith('ollama:') ? 768 : 512;
    const total = model === 'dinov2-vits14' ? 8 : model.startsWith('cohere:') ? 7 : model.startsWith('openai:') ? 6 : model.startsWith('ollama:') ? 5 : model === 'gemini-embedding-2' ? 0 : 12;
    const ids = Array.from({ length: Math.min(args?.limit ?? total, total) }, (_, i) => `img-${i}`);
    const vectors = ids.flatMap((_, imageIndex) =>
      Array.from({ length: dims }, (_, dimIndex) => Math.sin((imageIndex + 1) * (dimIndex + 1)) * 0.1)
    );
    return { ids, vectors, dims, total, offset: 0, limit: args?.limit ?? total, has_more: false };
  },
  list_jobs: () => [],
  get_job: () => null,
  cancel_job: () => undefined,
  pause_job: () => undefined,
  resume_job: () => undefined,
  set_rating: () => undefined,
  set_decision: () => undefined,
  undo: () => 'rating',
  redo: () => 'rating',
  cancel_claude_agent_chat_turn: () => true,
  trash_images: (_: any, args: { imageIds: string[] }) => args.imageIds.length,
  delete_images_permanently: (_: any, args: { imageIds: string[] }) => args.imageIds.length,
  rotate_image: (_: any, args: { imageId: string }) => `/mock/library/${args.imageId}_rotated.png`,
  crop_image: (_: any, args: { imageId: string }) => `/mock/library/${args.imageId}_crop.png`,
  get_generation_run: () => null,
  get_image_histogram: (_: any, args: { imageId: string }) => ({
    image_id: args.imageId,
    source: 'thumbnail',
    pixel_count: 1024,
    red: Array.from({ length: 256 }, (_, index) => index),
    green: Array.from({ length: 256 }, (_, index) => 255 - index),
    blue: Array.from({ length: 256 }, (_, index) => (index % 32) * 8),
    luma: Array.from({ length: 256 }, (_, index) => (index < 128 ? index : 255 - index)),
  }),
  record_asset_load_event: (_: any, args: { event: any }) => ({
    id: 'asset-event-1',
    ...args.event,
    created_at: '2026-05-01T12:00:00Z',
  }),
  get_asset_load_events: () => [],
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
    url: 'https://huggingface.co/Qdrant/clip-ViT-B-32-vision/resolve/e0c24ed0fa57fa3e4f97f30de74c51d944036ace/model.onnx',
    expected_sha256: 'c68d3d9a200ddd2a8c8a5510b576d4c94d1ae383bf8b36dd8c084f94e1fb4d63',
    expected_size_bytes: 351686194,
    spdx_license: 'MIT',
    source_repo: 'https://huggingface.co/Qdrant/clip-ViT-B-32-vision',
    model_card_url: 'https://huggingface.co/Qdrant/clip-ViT-B-32-vision',
    model_path: '/mock/app-data/models/clip-vit-b32-vision.onnx',
    part_path: '/mock/app-data/models/clip-vit-b32-vision.onnx.part',
    curl_command: "mkdir -p '/mock/app-data/models' && curl -L -C - -o '/mock/app-data/models/clip-vit-b32-vision.onnx.part' 'https://huggingface.co/Qdrant/clip-ViT-B-32-vision/resolve/e0c24ed0fa57fa3e4f97f30de74c51d944036ace/model.onnx' && test \"$(wc -c < '/mock/app-data/models/clip-vit-b32-vision.onnx.part' | tr -d '[:space:]')\" = '351686194' && printf '%s\\n' 'c68d3d9a200ddd2a8c8a5510b576d4c94d1ae383bf8b36dd8c084f94e1fb4d63  /mock/app-data/models/clip-vit-b32-vision.onnx.part' | shasum -a 256 -c - && mv '/mock/app-data/models/clip-vit-b32-vision.onnx.part' '/mock/app-data/models/clip-vit-b32-vision.onnx'",
  }),
  get_embedding_model_download_info: (_: any, args: { model: string }) => ({
    model_id: args.model,
    url: args.model === 'dinov2-vits14'
      ? 'https://huggingface.co/sefaburak/dinov2-small-onnx/resolve/7a5e61628117b5a8bd6f5e2b2385b76da1b4582e/dinov2_vits14.onnx'
      : 'https://huggingface.co/Qdrant/clip-ViT-B-32-vision/resolve/e0c24ed0fa57fa3e4f97f30de74c51d944036ace/model.onnx',
    expected_sha256: args.model === 'dinov2-vits14'
      ? '4df36ef0716a8f17d984fc7546a3a5d670fda6911eb298592250cb9e26756063'
      : 'c68d3d9a200ddd2a8c8a5510b576d4c94d1ae383bf8b36dd8c084f94e1fb4d63',
    expected_size_bytes: args.model === 'dinov2-vits14' ? 86644121 : 351686194,
    spdx_license: args.model === 'dinov2-vits14' ? 'Apache-2.0' : 'MIT',
    source_repo: args.model === 'dinov2-vits14'
      ? 'https://huggingface.co/sefaburak/dinov2-small-onnx'
      : 'https://huggingface.co/Qdrant/clip-ViT-B-32-vision',
    model_card_url: args.model === 'dinov2-vits14'
      ? 'https://huggingface.co/sefaburak/dinov2-small-onnx'
      : 'https://huggingface.co/Qdrant/clip-ViT-B-32-vision',
    model_path: `/mock/app-data/models/${args.model}.onnx`,
    part_path: `/mock/app-data/models/${args.model}.onnx.part`,
    curl_command: `curl -L -C - -o '/mock/app-data/models/${args.model}.onnx.part'`,
  }),
  list_embedding_providers: () => [
    {
      id: 'clip',
      label: 'CLIP ViT-B/32',
      shortLabel: 'CLIP',
      modelName: 'clip-vit-b32',
      dimensions: 512,
      dimensionsLabel: '512d',
      scope: 'local',
      runtime: 'local-onnx',
      status: 'ready',
      available: true,
      downloadable: true,
      downloadLabel: 'Download CLIP (~350MB)',
      expectedSha256: 'c68d3d9a200ddd2a8c8a5510b576d4c94d1ae383bf8b36dd8c084f94e1fb4d63',
      expectedSizeBytes: 351686194,
      spdxLicense: 'MIT',
      sourceRepo: 'https://huggingface.co/Qdrant/clip-ViT-B-32-vision',
      modelCardUrl: 'https://huggingface.co/Qdrant/clip-ViT-B-32-vision',
      apiKeyProvider: null,
    },
    {
      id: 'dinov2',
      label: 'DINOv2 ViT-S/14',
      shortLabel: 'DINOv2',
      modelName: 'dinov2-vits14',
      dimensions: 384,
      dimensionsLabel: '384d',
      scope: 'local',
      runtime: 'local-onnx',
      status: 'ready',
      available: true,
      downloadable: true,
      downloadLabel: 'Download DINOv2 (~87MB)',
      expectedSha256: '4df36ef0716a8f17d984fc7546a3a5d670fda6911eb298592250cb9e26756063',
      expectedSizeBytes: 86644121,
      spdxLicense: 'Apache-2.0',
      sourceRepo: 'https://huggingface.co/sefaburak/dinov2-small-onnx',
      modelCardUrl: 'https://huggingface.co/sefaburak/dinov2-small-onnx',
      apiKeyProvider: null,
    },
    {
      id: 'gemini',
      label: 'Gemini Embedding 2',
      shortLabel: 'Gemini',
      modelName: 'gemini-embedding-2',
      dimensions: 3072,
      dimensionsLabel: '3072d',
      scope: 'cloud',
      runtime: 'cloud-api',
      status: 'key',
      available: false,
      downloadable: false,
      downloadLabel: null,
      expectedSha256: null,
      expectedSizeBytes: null,
      spdxLicense: null,
      sourceRepo: null,
      modelCardUrl: null,
      apiKeyProvider: 'google',
    },
    {
      id: 'cohere',
      label: 'Cohere Embed v4 Multimodal',
      shortLabel: 'Cohere',
      modelName: 'cohere:embed-v4.0',
      dimensions: 1024,
      dimensionsLabel: '1024d',
      scope: 'cloud',
      runtime: 'cloud-api',
      status: 'key',
      available: false,
      downloadable: false,
      downloadLabel: null,
      expectedSha256: null,
      expectedSizeBytes: null,
      spdxLicense: null,
      sourceRepo: null,
      modelCardUrl: null,
      apiKeyProvider: 'cohere',
    },
    {
      id: 'openai',
      label: 'OpenAI Text Embedding 3 Large',
      shortLabel: 'OpenAI',
      modelName: 'openai:text-embedding-3-large',
      dimensions: 3072,
      dimensionsLabel: '3072d',
      scope: 'cloud',
      runtime: 'cloud-api',
      status: 'key',
      available: false,
      downloadable: false,
      downloadLabel: null,
      expectedSha256: null,
      expectedSizeBytes: null,
      spdxLicense: null,
      sourceRepo: null,
      modelCardUrl: null,
      apiKeyProvider: 'openai',
    },
    {
      id: 'ollama',
      label: 'Ollama Text Embeddings',
      shortLabel: 'Ollama',
      modelName: 'ollama:embeddinggemma',
      dimensions: 0,
      dimensionsLabel: 'model',
      scope: 'local',
      runtime: 'local-api',
      status: 'ready',
      available: true,
      downloadable: false,
      downloadLabel: null,
      expectedSha256: null,
      expectedSizeBytes: null,
      spdxLicense: null,
      sourceRepo: null,
      modelCardUrl: null,
      apiKeyProvider: null,
    },
  ],
  check_ollama_embedding: () => ['embeddinggemma:latest', 'nomic-embed-text:latest'],
  get_ollama_embedding_config: () => ['http://localhost:11434/api/embed', 'embeddinggemma'],
  set_ollama_embedding_config: () => undefined,
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
  delete_folder: () => 0,
  list_collections: () => mockCollections,
  create_collection: (_: any, args: { name: string }) => {
    const id = `col-${nextId++}`;
    mockCollections.push([id, args.name, 0]);
    return id;
  },
  add_to_collection: () => undefined,
  remove_from_collection: () => undefined,
  delete_collection: (_: any, args: { collectionId: string }) => {
    const index = mockCollections.findIndex(([id]) => id === args.collectionId);
    if (index >= 0) mockCollections.splice(index, 1);
  },
  list_images_by_folder: () => [],
  list_images_filtered: () => [],
  list_collection_images: (_: any, args: { collectionId: string }) =>
    args.collectionId === 'col_clipboard_mock' ? [makeMockImage(0), makeMockImage(1)] : [],
  is_yolo_available: () => true,
  is_nudenet_available: () => true,
  download_yolo_model: () => {
    throw new Error('Built-in YOLO downloads are disabled for the Apache-2.0 release.');
  },
  download_nudenet_model: () => {
    throw new Error('Built-in NudeNet downloads are disabled for the Apache-2.0 release.');
  },
  detect_objects: (_: any, args: { imageIds: string[] }) => args.imageIds.length,
  detect_nsfw: (_: any, args: { imageIds: string[] }) => args.imageIds.length,
  get_detection_count: (_: any, args: { model: string }) => args.model === 'yolo11m' ? 5 : 1,
  count_by_detected_class: (_: any, args: { className: string }) => args.className === 'person' ? 5 : 0,
  search_by_detected_class: () => [['img-0', 0.95], ['img-1', 0.9]],
  list_images_by_detected_class: () => [makeMockImage(0), makeMockImage(1)],
  get_detections: () => [
    { class_name: 'person', confidence: 0.93, x: 0.16, y: 0.18, width: 0.32, height: 0.56 },
    { class_name: 'EXPOSED_BREAST_F', confidence: 0.88, x: 0.52, y: 0.22, width: 0.2, height: 0.28 },
  ],
  check_ollama: () => [],
  get_ollama_config: () => ['http://localhost:11434/api/generate', 'llava'],
  set_ollama_config: () => undefined,
  analyze_images: (_: any, args: { imageIds: string[] }) => args.imageIds.length,
  get_vision_metadata: () => [],
  get_vision_count: () => 0,
  rescan_sources: () => 20,
  list_lineage_groups: () => [],
  get_lineage_group_images: () => [],
  rename_lineage_group: () => undefined,
  dissolve_lineage_group: () => undefined,
  list_sessions: () => mockSessions,
  create_session: (_: any, args: { name: string }) => ({
    ...mockSessions[0],
    id: `session-${nextId++}`,
    name: args.name,
    image_count: 0,
  }),
  validate_session_folder: () => true,
  list_canvases: () => mockCanvases,
  create_canvas: (_: any, args: { sessionId: string; name: string; canvasType: string }) => ({
    ...mockCanvases[0],
    id: `canvas-${nextId++}`,
    session_id: args.sessionId,
    name: args.name,
    canvas_type: args.canvasType,
  }),
  update_canvas_layout: () => undefined,
  delete_canvas: () => undefined,
  list_mcp_tokens: () => [],
  get_mcp_status: () => ({ active_connections: 0 }),
  create_mcp_token: () => [
    { id: 'token-1', name: 'Smoke', role: 'viewer', scope_json: null, created_at: '2026-05-01T12:00:00Z', expires_at: '2026-07-30T12:00:00Z', last_used_at: null, revoked_at: null },
    'cull_test_secret',
  ],
  revoke_mcp_token: () => undefined,
  rotate_mcp_token: () => 'cull_test_secret_rotated',
  get_data_flow_status: () => [],
  get_api_audit_log: () => [],
  export_audit_log: () => '/mock/audit.json',
  backfill_raw_previews: () => 0,
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

export function convertFileSrc(filePath: string): string {
  return mockImageDataUri(filePath);
}

export async function listen<T>(event: string, handler: MockListener<T>, _options?: any): Promise<UnlistenFn> {
  const id = nextListenerId++;
  const listeners = mockListeners.get(event) ?? new Map<number, MockListener>();
  listeners.set(id, handler as MockListener);
  mockListeners.set(event, listeners);
  return () => {
    listeners.delete(id);
    if (listeners.size === 0) mockListeners.delete(event);
  };
}

export async function getCurrent(): Promise<string[]> {
  return [];
}

export async function onOpenUrl(handler: (urls: string[]) => void): Promise<UnlistenFn> {
  return listen<string[]>('deep-link-open-url', (event) => handler(event.payload));
}

export async function check(): Promise<null> {
  return null;
}

export async function relaunch(..._args: any[]): Promise<void> {
  return undefined;
}

export async function open(..._args: any[]): Promise<string | string[] | null> {
  return null;
}

// Native save dialog (@tauri-apps/plugin-dialog). Returns null so E2E export
// flows treat it as a cancelled dialog rather than crashing.
export async function save(..._args: any[]): Promise<string | null> {
  return null;
}

export async function openPath(..._args: any[]): Promise<void> {
  return undefined;
}

export async function openUrl(..._args: any[]): Promise<void> {
  return undefined;
}

export async function revealItemInDir(..._args: any[]): Promise<void> {
  return undefined;
}
