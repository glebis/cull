import { configDefaults, defineConfig } from "vitest/config";
import { sveltekit } from "@sveltejs/kit/vite";

const host = process.env.TAURI_DEV_HOST;
const e2eMock = process.env.CULL_E2E_MOCK === "1";
const tauriMock = decodeURIComponent(new URL("./src/lib/tauri-mock.ts", import.meta.url).pathname);
const tauriMockAliases = [
  "@tauri-apps/api/core",
  "@tauri-apps/api/event",
  "@tauri-apps/plugin-deep-link",
  "@tauri-apps/plugin-dialog",
  "@tauri-apps/plugin-opener",
  "@tauri-apps/plugin-process",
  "@tauri-apps/plugin-updater",
].map((find) => ({ find, replacement: tauriMock }));

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [sveltekit()],
  resolve: e2eMock ? { alias: tauriMockAliases } : undefined,
  test: {
    exclude: [...configDefaults.exclude, "**/.worktrees/**"],
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
