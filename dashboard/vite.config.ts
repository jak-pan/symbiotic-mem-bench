import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// No SSR — a plain SPA built to dashboard/dist, which the Rust server serves.
// In dev, Vite proxies /api to the membench-server backend.
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
