// diadOS first-boot setup wizard bundle.
//
// Mirrors the lockscreen build: esbuild IIFE bundle + a static index.html
// served straight off the ISO filesystem (no Node runtime inside the ISO —
// this bundle is built on the *build host* by `make iso`).

import { execSync } from "node:child_process";
import { cpSync, mkdirSync } from "node:fs";
import { build } from "esbuild";

// Typecheck first (strict) — the repo gates on zero TS errors.
execSync("npx tsc --noEmit -p tsconfig.json", { stdio: "inherit" });

const DIST = "../archiso/airootfs/usr/share/diad/setup/dist";
mkdirSync(DIST, { recursive: true });

await build({
  entryPoints: ["src/setup-app.tsx"],
  bundle: true,
  minify: true,
  loader: {
    ".png": "dataurl",
    ".svg": "dataurl",
  },
  outfile: `${DIST}/app.js`,
  jsx: "automatic",
  format: "iife",
  target: ["chrome120"],
  define: { "process.env.NODE_ENV": '"production"' },
  logLevel: "info",
});

cpSync("index.html", `${DIST}/index.html`);

console.log(`setup wizard bundle written to ${DIST}`);
