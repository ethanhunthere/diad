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
    // Chunk splitting (spec 6.1/6.2): framework vendor code is isolated so
    // the app CORE chunk stays under budget; every panel is its own lazy
    // chunk and drags its heavy libs (CodeMirror, xterm, markdown) with it.
    rollupOptions: {
      output: {
        manualChunks: {
          "vendor-react": ["react", "react-dom", "react/jsx-runtime"],
          "vendor-zustand": ["zustand"],
        },
      },
    },
  },
  server: {
    port: 5173,
    strictPort: false,
  },
});
