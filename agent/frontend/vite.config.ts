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
    // Split heavy editor/terminal/UI vendors into their own chunks so the
    // app shell loads first (served locally by the daemon over HTTP/1.1).
    rollupOptions: {
      output: {
        manualChunks: {
          "vendor-react": ["react", "react-dom", "zustand"],
          "vendor-editor": [
            "@uiw/react-codemirror",
            "@codemirror/lang-css",
            "@codemirror/lang-html",
            "@codemirror/lang-javascript",
            "@codemirror/lang-json",
            "@codemirror/lang-markdown",
            "@codemirror/lang-python",
            "@codemirror/lang-rust",
            "@codemirror/lang-sql",
            "@codemirror/theme-one-dark",
          ],
          "vendor-xterm": ["@xterm/xterm", "@xterm/addon-fit"],
          "vendor-markdown": ["react-markdown", "remark-gfm", "rehype-highlight"],
        },
      },
    },
  },
  server: {
    port: 5173,
    strictPort: false,
  },
});
