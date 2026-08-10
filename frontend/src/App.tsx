import { useEffect, useRef, useState } from "react";
import { Menu } from "lucide-react";
import { useStore } from "./stores/useStore";
import { useWebSocket } from "./hooks/useWebSocket";
import { TopBar } from "./components/TopBar";
import { ActivityBar, VIEWS } from "./components/ActivityBar";
import { ChatPanel } from "./panels/ChatPanel";
import { ExplorerPanel } from "./panels/ExplorerPanel";
import { EditorPanel } from "./panels/EditorPanel";
import { TerminalPanel } from "./panels/TerminalPanel";
import { AgentsPanel } from "./panels/AgentsPanel";
import { SettingsPanel } from "./panels/SettingsPanel";
import { GitPanel } from "./panels/GitPanel";
import { DebugPanel } from "./panels/DebugPanel";
import { PreviewPanel } from "./panels/PreviewPanel";
import { ProjectsPanel } from "./panels/ProjectsPanel";
import { StatusBar } from "./components/StatusBar";
import { CommandPalette } from "./components/CommandPalette";
import { ToastStack } from "./components/ToastStack";
import { OnboardingWizard } from "./panels/OnboardingWizard";
import { MaskedInputModal } from "./components/MaskedInputModal";
import { ConfirmModal } from "./components/ConfirmModal";
import { useGlobalKeys } from "./hooks/useGlobalKeys";
import { VIEW_LABELS } from "./lib/format";

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
            {VIEWS.map(({ id, icon: Icon }) => (
              <button
                key={id}
                className={`flex min-h-[44px] items-center gap-2.5 rounded-control border border-border-subtle px-3 py-2 text-left text-[13px] ${
                  view === id ? "bg-copper/15 text-copper-hover" : "bg-surface-2/50 text-dim"
                }`}
                onClick={() => {
                  setView(id);
                  setMobileNav(false);
                }}
              >
                <Icon size={16} /> {VIEW_LABELS[id]}
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
