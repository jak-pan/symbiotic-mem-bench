import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// No SSR — a plain SPA built to dashboard/dist, which the Rust server serves.
// In dev, Vite proxies /api to the membench-server backend. The bundle content
// hash + git sha are stamped into dist/version.json by scripts/write-version.mjs
// after the build (see package.json `build`).
export default defineConfig({
  plugins: [svelte()],
  server: {
    port: 5173,
    proxy: {
      "/api": "http://127.0.0.1:8787",
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "es2022",
  },
});
