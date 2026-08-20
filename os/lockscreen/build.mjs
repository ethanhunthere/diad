// diadOS web lockscreen bundle.
//
// Bundles the Originkit "character-cursor" component (shipped by the CLI in
// the airootfs tree) plus the unlock UI into a single static `dist/` folder
// that the ISO serves straight off the filesystem (no Node runtime in the
// ISO — this bundle is built on the *build host* by `make iso`).

import { execSync } from "node:child_process";
import { cpSync, mkdirSync } from "node:fs";
import { build } from "esbuild";

// Typecheck first (strict) — the repo gates on zero TS errors.
execSync("npx tsc --noEmit -p tsconfig.json", { stdio: "inherit" });

const DIST = "../archiso/airootfs/usr/share/diad/lockscreen/dist";
mkdirSync(DIST, { recursive: true });

await build({
  entryPoints: ["src/lock-app.tsx"],
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

console.log(`lockscreen bundle written to ${DIST}`);
