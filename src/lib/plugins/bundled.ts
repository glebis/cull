// First-party plugins compiled into the app build. Activated at startup,
// before registry plugins, regardless of the module_plugins flag.
import cullPublish from './cull-publish';
import type { BundledPlugin } from './loader';

export const BUNDLED_PLUGINS: BundledPlugin[] = [cullPublish];
