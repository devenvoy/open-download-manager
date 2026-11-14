import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

// Open Download Manager frontend — React 19 + Vite + Tailwind CSS v4 + shadcn/ui.
// The dev server runs on :5173 so Tauri can connect via devUrl in production builds.
export default defineConfig({
  root: import.meta.dirname,
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(import.meta.dirname, "./src"),
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    sourcemap: false,
    rollupOptions: {
      // Two windows, two entries: the main/aux windows load index.html, the
      // Torrents window loads torrents.html (window_manager.rs opens both).
      input: {
        main: path.resolve(import.meta.dirname, "index.html"),
        torrents: path.resolve(import.meta.dirname, "torrents.html"),
      },
    },
  },
  server: {
    port: 5173,
    strictPort: true,
    fs: {
      // Allow Tauri to load the dev server assets during `tauri dev`.
      allow: [".."],
    },
  },
});
