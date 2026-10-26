import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

// The dev server is only used by `npm run tauri dev`. Release builds embed `dist/`
// in the app, which then opens no port at all.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 5173,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: { outDir: "dist", emptyOutDir: true, target: "es2022" },
});
