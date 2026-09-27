import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vitest/config";

const debug = !!process.env.TAURI_ENV_DEBUG;

// https://v2.tauri.app/start/frontend/vite/
export default defineConfig({
  plugins: [svelte()],
  // Use Svelte's client runtime in tests so runes behave as in the app.
  resolve: process.env.VITEST ? { conditions: ["browser"] } : undefined,
  // Keep Rust errors visible in the terminal.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    minify: !debug,
    sourcemap: debug,
  },
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
});
