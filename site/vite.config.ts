import { defineConfig } from "vite";

export default defineConfig({
  build: {
    sourcemap: true,
  },
  server: {
    port: 4175,
    strictPort: true,
  },
});
