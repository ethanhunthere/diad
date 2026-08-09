import type { ViewId } from "./types";

/** Tiny monotonic id generator for frontend-local requests. */
let counter = 0;
export function nextId(prefix = "gui"): string {
  counter += 1;
  return `${prefix}-${Date.now().toString(36)}-${counter.toString(36)}`;
}

/** 12-hour clock used by the top bar. */
export function clock12(d = new Date()): string {
  let h = d.getHours();
  const m = d.getMinutes().toString().padStart(2, "0");
  const ampm = h >= 12 ? "PM" : "AM";
  h = h % 12 || 12;
  return `${h}:${m} ${ampm}`;
}

/** Human-readable byte size. */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

/** Truncate a long path for display. */
export function shortenPath(p: string, max = 44): string {
  if (p.length <= max) return p;
  const parts = p.split("/");
  let out = parts[parts.length - 1];
  let i = parts.length - 2;
  while (i >= 0 && out.length < max - 3) {
    out = `${parts[i]}/${out}`;
    i -= 1;
  }
  return out.length < p.length ? `…${out.slice(Math.max(0, out.length - (max - 1)))}` : p;
}

/** Basename of a path. */
export function basename(p: string): string {
  const parts = p.split("/");
  return parts[parts.length - 1] || p;
}

export const VIEW_LABELS: Record<ViewId, string> = {
  chat: "Chat",
  explorer: "Explorer",
  editor: "Editor",
  terminal: "Terminal",
  agents: "Agents",
  settings: "Settings",
  git: "Git",
  debug: "Debug",
  preview: "Preview",
  projects: "Projects",
};
