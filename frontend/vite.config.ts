import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Relative base so the built bundle works when served from the daemon
// (rust-embed → http://127.0.0.1:7860/) without a <base> tag.
export default defineConfig({
  base: "./",
  plugins: [react()],
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "es2020",
    sourcemap: false,
  },
  server: {
    port: 5173,
    strictPort: false,
  },
});
