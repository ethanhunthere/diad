#!/usr/bin/env node
// ============================================================================
// DIAD AGENT — bundle budget gate (spec 6.3)
// ============================================================================
// Prints the gzipped size table for every JS chunk in dist/assets and FAILS
// the build when a chunk exceeds its budget:
//
//   core (entry)        < 150 KB gzipped
//   editor panel        < 200 KB gzipped   (CodeMirror is heavy by nature)
//   terminal panel      < 120 KB gzipped   (xterm.js)
//   chat panel          < 120 KB gzipped   (markdown + highlight)
//   every other chunk   <  80 KB gzipped
//
import { gzipSync } from "node:zlib";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const ASSETS = join(import.meta.dirname, "..", "dist", "assets");

/** Budget in gzipped bytes per chunk-name pattern. */
const BUDGETS = [
  { match: /EditorPanel/i, name: "editor", limit: 200 * 1024 },
  { match: /TerminalPanel/i, name: "terminal", limit: 120 * 1024 },
  { match: /ChatPanel/i, name: "chat", limit: 120 * 1024 },
];
const CORE_LIMIT = 150 * 1024;
const OTHER_LIMIT = 80 * 1024;

const files = readdirSync(ASSETS).filter((f) => f.endsWith(".js"));
if (files.length === 0) {
  console.error("bundle-check: no JS chunks found in dist/assets — run vite build first.");
  process.exit(1);
}

// The entry chunk is the one index.html actually loads; lazy/vendor chunks
// are everything else (many also start with "index-" — dynamic imports of
// packages' index modules).
const indexHtml = readFileSync(join(import.meta.dirname, "..", "dist", "index.html"), "utf8");
const entryNames = new Set(
  [...indexHtml.matchAll(/assets\/(.+?\.js)/g)].map((m) => m[1]),
);

const rows = [];
let failed = false;

for (const file of files.sort()) {
  const raw = readFileSync(join(ASSETS, file));
  const gz = gzipSync(raw).length;
  const isEntry = entryNames.has(file);

  let label = isEntry ? "core (entry)" : "other";
  let limit = isEntry ? CORE_LIMIT : OTHER_LIMIT;
  for (const b of BUDGETS) {
    if (b.match.test(file)) {
      label = b.name;
      limit = b.limit;
      break;
    }
  }

  const ok = gz <= limit;
  if (!ok) failed = true;
  rows.push({
    chunk: file.slice(0, 46),
    label,
    kb: (gz / 1024).toFixed(1),
    budget: (limit / 1024).toFixed(0),
    status: ok ? "ok" : "OVER BUDGET",
  });
}

console.log("\n=== DIAD AGENT bundle sizes (gzipped) ===");
console.table(rows);

if (failed) {
  console.error("bundle-check: FAILED — at least one chunk exceeds its budget (spec 6.3).");
  process.exit(1);
}
console.log("bundle-check: all chunks within budget.");
