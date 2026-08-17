import { lazy, Suspense, useEffect, useRef, useState } from "react";
import type { ComponentType } from "react";
import { Menu } from "lucide-react";
import { useStore } from "./stores/useStore";
import { useWebSocket } from "./hooks/useWebSocket";
import { TopBar } from "./components/TopBar";
import { ActivityBar, PRIMARY_VIEWS } from "./components/ActivityBar";
import { StatusBar } from "./components/StatusBar";
import { CommandPalette } from "./components/CommandPalette";
import { ToastStack } from "./components/ToastStack";
import { OnboardingWizard } from "./panels/OnboardingWizard";
import { WorkspaceHub } from "./panels/WorkspaceHub";
import { MaskedInputModal } from "./components/MaskedInputModal";
import { ConfirmModal } from "./components/ConfirmModal";
import { useGlobalKeys } from "./hooks/useGlobalKeys";

import { prefetchAggressive, setPerfTier } from "./lib/prefetch";

// Lazy-loaded panels (spec 6.1/6.2): each panel is its own chunk and drags
// its heavy libs with it — CodeMirror with EditorPanel, xterm with
// TerminalPanel, the markdown renderer with ChatPanel. Nothing here loads
// until the view is opened (LOW/ULTRA_LOW tiers) or prefetched (MID+).
// The panels keep their named exports; the then() maps them to `default`.
const lazyNamed = <T,>(load: () => Promise<T>, pick: (m: T) => unknown) =>
  lazy(() => load().then((m) => ({ default: pick(m) as ComponentType })));

const ChatPanel = lazyNamed(() => import("./panels/ChatPanel"), (m) => m.ChatPanel);
const ExplorerPanel = lazyNamed(() => import("./panels/ExplorerPanel"), (m) => m.ExplorerPanel);
const EditorPanel = lazyNamed(() => import("./panels/EditorPanel"), (m) => m.EditorPanel);
const TerminalPanel = lazyNamed(() => import("./panels/TerminalPanel"), (m) => m.TerminalPanel);
const AgentsPanel = lazyNamed(() => import("./panels/AgentsPanel"), (m) => m.AgentsPanel);
const SettingsPanel = lazyNamed(() => import("./panels/SettingsPanel"), (m) => m.SettingsPanel);
const GitPanel = lazyNamed(() => import("./panels/GitPanel"), (m) => m.GitPanel);
const DebugPanel = lazyNamed(() => import("./panels/DebugPanel"), (m) => m.DebugPanel);
const PreviewPanel = lazyNamed(() => import("./panels/PreviewPanel"), (m) => m.PreviewPanel);
const ProjectsPanel = lazyNamed(() => import("./panels/ProjectsPanel"), (m) => m.ProjectsPanel);
const ClipboardPanel = lazyNamed(() => import("./panels/ClipboardPanel"), (m) => m.ClipboardPanel);
const TrashPanel = lazyNamed(() => import("./panels/TrashPanel"), (m) => m.TrashPanel);

/** Suspense fallback while a lazy panel chunk streams in. */
function PanelLoading() {
  return (
    <div className="flex h-full items-center justify-center text-[12px] text-faint">
      <span className="streaming-dot mr-2 inline-block h-2 w-2 rounded-full bg-copper" />
      loading panel…
    </div>
  );
}

/**
 * GUI breakpoints (spec 2.1) — the exact thresholds mirrored from
 * `core/perf.rs::breakpoint` (covered by Rust unit tests).
 */
function breakpointFor(width: number): "TINY" | "COMPACT" | "NORMAL" | "WIDE" | "ULTRAWIDE" {
  if (width < 800) return "TINY";
  if (width < 1280) return "COMPACT";
  if (width < 1920) return "NORMAL";
  if (width < 3000) return "WIDE";
  return "ULTRAWIDE";
}

/**
 * DIAD AGENT — the graphical face of the DIAD intent-driven OS.
 * Speaks protocol v1 over WebSocket to the diad-agent daemon.
 *
 * Responsiveness (spec section 2): a ResizeObserver re-evaluates the
 * breakpoint on EVERY resize and publishes it as `data-bp` on <html>,
 * where the stylesheet adapts the layout. No fixed pixel widths — the
 * layout is flex with fr/minmax throughout and survives down to 640x480.
 */
export default function App() {
  useWebSocket();
  useGlobalKeys();

  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);
  const activityBarCollapsed = useStore((s) => s.activityBarCollapsed);
  const pendingSecret = useStore((s) => s.pendingSecret);
  const pendingConfirm = useStore((s) => s.pendingConfirm);
  const onboarding = useStore((s) => s.onboarding);
  const paletteOpen = useStore((s) => s.paletteOpen);
  const fontScale = useStore((s) => s.settings.fontScale);
  const density = useStore((s) => s.settings.density);

  const rootRef = useRef<HTMLDivElement>(null);
  const [mobileNav, setMobileNav] = useState(false);

  // Live breakpoint evaluation on every resize (spec 2.2).
  useEffect(() => {
    const el = rootRef.current;
    if (!el) return;
    const apply = (w: number) => {
      document.documentElement.dataset.bp = breakpointFor(w);
    };
    apply(el.clientWidth);
    const ro = new ResizeObserver((entries) => {
      for (const e of entries) apply(e.contentRect.width);
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Touch adaptation (spec 2.7): hit targets grow, hover states drop.
  useEffect(() => {
    if (navigator.maxTouchPoints > 0) {
      document.documentElement.classList.add("touch");
    }
  }, []);

  const theme = useStore((s) => s.settings.theme);

  // Theme synchronization (2026 clean design system):
  // Updates data-theme attribute on root element live without reloading.
  useEffect(() => {
    const t = theme || localStorage.getItem("diad.theme") || "monochrome";
    document.documentElement.setAttribute("data-theme", t);
    localStorage.setItem("diad.theme", t);
  }, [theme]);

  // Font scaling (spec 2.9): rem-based UI follows the root font size; the
  // multiplier persists in localStorage and applies without a restart.
  useEffect(() => {
    document.documentElement.style.fontSize = `${(16 * fontScale).toFixed(1)}px`;
    localStorage.setItem("diad.fontScale", String(fontScale));
  }, [fontScale]);

  // Density override (spec 2.7): auto | compact | comfortable.
  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("compact", "comfortable");
    if (density !== "auto") root.classList.add(density);
    localStorage.setItem("diad.density", density);
  }, [density]);

  // Tier-aware prefetch (spec 6.4): learn the daemon's perf tier, then warm
  // the heavy chunks only on tiers that can afford it.
  useEffect(() => {
    import("./services/api")
      .then(({ apiGet }) => apiGet<{ perf_tier?: string }>("/status"))
      .then((status) => {
        setPerfTier(status.perf_tier ?? "MID");
        prefetchAggressive();
      })
      .catch(() => {
        /* daemon offline — MID defaults apply */
      });
  }, []);

  // PROMPT-2 S4.3 — GUI fallback toast: if the previous boot's GUI failed
  // twice and the supervisor engaged the TUI, surface the persisted reason
  // exactly once (GET /gui/fallback then DELETE clears the marker).
  useEffect(() => {
    if (window.location.pathname !== "/") return;
    import("./services/api")
      .then(({ apiGet, api }) =>
        apiGet<{ reason: string | null }>("/gui/fallback").then((r) => {
          if (r.reason) {
            useStore.getState().toast({
              kind: "warn",
              title: "GUI fallback",
              message: r.reason,
              ttl: 8000,
            });
            api("/gui/fallback", { method: "DELETE" }).catch(() => {});
          }
        }),
      )
      .catch(() => {
        /* daemon offline — nothing to surface */
      });
  }, []);

  const path = window.location.pathname;
  const isWorkspaceHub = path.startsWith("/workspace") || path === "/desktop" || path === "/launch" || new URLSearchParams(window.location.search).has("ws");

  if (isWorkspaceHub) {
    return <WorkspaceHub />;
  }

  return (
    <div ref={rootRef} className="flex h-full flex-col bg-obsidian text-ink">
      <TopBar
        hamburger={
          <button
            className="icon-btn hamburger"
            onClick={() => setMobileNav(true)}
            title="Menu"
          >
            <Menu size={16} />
          </button>
        }
      />
      <div className="flex min-h-0 flex-1">
        <ActivityBar collapsed={activityBarCollapsed} />
        <main className="min-w-0 flex-1 overflow-hidden">
          <Suspense fallback={<PanelLoading />}>
            {view === "chat" && <ChatPanel />}
            {view === "explorer" && <ExplorerPanel />}
            {view === "editor" && <EditorPanel />}
            {view === "terminal" && <TerminalPanel />}
            {view === "agents" && <AgentsPanel />}
            {view === "settings" && <SettingsPanel />}
            {view === "git" && <GitPanel />}
            {view === "debug" && <DebugPanel />}
            {view === "preview" && <PreviewPanel />}
            {view === "projects" && <ProjectsPanel />}
            {view === "clipboard" && <ClipboardPanel />}
            {view === "trash" && <TrashPanel />}
          </Suspense>
        </main>
      </div>
      <StatusBar />

      {/* TINY breakpoint: activity bar is replaced by this overlay menu. */}
      {mobileNav && (
        <div
          className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm animate-fade-in"
          onMouseDown={() => setMobileNav(false)}
        >
          <nav
            className="glass m-3 grid grid-cols-2 gap-2 rounded-panel p-3"
            onMouseDown={(e) => e.stopPropagation()}
          >
            {PRIMARY_VIEWS.map(({ id, label, icon: Icon }) => (
              <button
                key={id}
                className={`flex min-h-[44px] items-center gap-2.5 rounded-control border border-border-subtle px-3 py-2 text-left text-[14px] ${
                  view === id ? "bg-copper/20 text-ink font-semibold" : "bg-surface-2/50 text-dim"
                }`}
                onClick={() => {
                  setView(id);
                  setMobileNav(false);
                }}
              >
                <Icon size={18} /> {label}
              </button>
            ))}
          </nav>
        </div>
      )}

      {/* Overlays */}
      {paletteOpen && <CommandPalette />}
      <ToastStack />
      {onboarding && <OnboardingWizard />}
      {pendingSecret && <MaskedInputModal />}
      {pendingConfirm && <ConfirmModal />}
    </div>
  );
}
