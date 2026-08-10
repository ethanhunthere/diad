// ============================================================================
// Tier-aware chunk prefetch (spec 6.4)
// ============================================================================
// HIGH / ULTRA : prefetch editor + terminal chunks on idle after first paint
// MID          : prefetch a chunk when its activity-bar icon is hovered
// LOW / ULTRA_LOW : no prefetch — load strictly on demand
//
// The daemon's perf tier arrives via GET /api/v1/status (`perf_tier`).

export type PerfTier = "ULTRA_LOW" | "LOW" | "MID" | "HIGH" | "ULTRA";

let currentTier: PerfTier = "MID";

/** Remember the boot tier (called once by App after fetching /status). */
export function setPerfTier(tier: string): void {
  if (["ULTRA_LOW", "LOW", "MID", "HIGH", "ULTRA"].includes(tier)) {
    currentTier = tier as PerfTier;
  }
}

export function getPerfTier(): PerfTier {
  return currentTier;
}

const loadEditor = () => import("../panels/EditorPanel");
const loadTerminal = () => import("../panels/TerminalPanel");

/** HIGH/ULTRA: warm the two heaviest chunks during browser idle time. */
export function prefetchAggressive(): void {
  if (currentTier !== "HIGH" && currentTier !== "ULTRA") return;
  const run = () => {
    void loadEditor();
    void loadTerminal();
  };
  if (typeof window.requestIdleCallback === "function") {
    window.requestIdleCallback(run);
  } else {
    setTimeout(run, 2000);
  }
}

/** MID: hover-driven prefetch of a single view's chunk. */
export function prefetchView(view: string): void {
  if (currentTier !== "MID") return;
  if (view === "editor") void loadEditor();
  else if (view === "terminal") void loadTerminal();
}
