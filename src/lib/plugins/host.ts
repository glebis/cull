// Plugin host API contract (v1, deliberately tiny).
//
// A plugin is a precompiled ESM bundle whose default export is
// `{ activate(host) }`. The host object is the ONLY surface a plugin gets:
// - mountView(el): hand the host a root element for the plugin's view
// - registerPaletteCommands([...]): contribute command-palette entries
// - invoke(tool, args): the single privileged path, enforced in Rust by the
//   plugin_invoke capability bridge (never checked webview-side)

export interface PluginManifest {
    id: string;
    name: string;
    version: string;
    description: string;
    entry: string;
    permissions: string[];
    minAppVersion: string;
    checksum: string;
    repo: string;
}

/** Shape returned by the Rust `load_installed_plugins` command: the manifest
 * plus the hash-verified ESM source (re-verified again in the webview). */
export interface LoadedPlugin {
    manifest: PluginManifest;
    source: string;
}

export interface PluginPaletteCommand {
    id: string;
    title: string;
    subtitle?: string;
    keywords?: string[];
    run: () => void | Promise<void>;
}

export interface PluginHost {
    /** Register the plugin's view root element. The app decides where and
     * when it is shown (no plugin view surface ships in runtime v1). */
    mountView(el: HTMLElement): void;
    registerPaletteCommands(commands: PluginPaletteCommand[]): void;
    /** The only privileged path: routes through the Rust plugin_invoke
     * command, which checks the plugin's granted capabilities. */
    invoke(tool: string, args?: Record<string, unknown>): Promise<unknown>;
}

export interface PluginModule {
    default?: { activate?: (host: PluginHost) => void | Promise<void> };
}

/** Data for the install-time consent dialog: every requested permission with
 * a human-readable description, shown BEFORE anything is granted. */
export interface GrantPromptModel {
    pluginId: string;
    name: string;
    permissions: Array<{ capability: string; description: string }>;
}
