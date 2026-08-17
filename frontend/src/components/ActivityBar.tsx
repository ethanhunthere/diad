import {
  MessageSquare,
  FolderOpen,
  TerminalSquare,
  Settings2,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react";
import { useStore } from "../stores/useStore";
import type { ViewId } from "../lib/types";
import { prefetchView } from "../lib/prefetch";

export const PRIMARY_VIEWS: { id: ViewId; label: string; icon: typeof MessageSquare }[] = [
  { id: "chat", label: "Agent Studio", icon: MessageSquare },
  { id: "explorer", label: "Project Files", icon: FolderOpen },
  { id: "terminal", label: "Terminal Console", icon: TerminalSquare },
  { id: "settings", label: "Settings & Themes", icon: Settings2 },
];

/**
 * Clean, simplified 56px activity bar with large accessible targets and clear tooltips.
 */
export function ActivityBar({ collapsed }: { collapsed: boolean }) {
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);
  const toggleActivityBar = useStore((s) => s.toggleActivityBar);
  const streaming = useStore((s) => s.streaming);

  if (collapsed) {
    return (
      <nav className="activity-bar flex w-0 shrink-0 flex-col items-center border-r border-border-subtle bg-surface/60 py-3 transition-all duration-150 ease-out">
        <button
          className="icon-btn h-9 w-9"
          onClick={toggleActivityBar}
          title="Expand sidebar"
          aria-label="Expand sidebar"
        >
          <PanelLeftOpen size={18} />
        </button>
      </nav>
    );
  }

  return (
    <nav className="activity-bar flex w-14 shrink-0 flex-col items-center border-r border-border-subtle bg-surface/60 py-3 transition-all duration-150 ease-out select-none">
      <div className="flex flex-col gap-2">
        {PRIMARY_VIEWS.map(({ id, label, icon: Icon }) => {
          const active = view === id;
          const isChatStreaming = id === "chat" && streaming;
          return (
            <button
              key={id}
              onClick={() => setView(id)}
              onMouseEnter={() => prefetchView(id)}
              title={label}
              aria-label={label}
              className={`relative flex h-11 w-11 items-center justify-center rounded-xl transition-all duration-150 ${
                active
                  ? "bg-copper/20 text-ink shadow-sm ring-1 ring-copper/40"
                  : "text-dim hover:bg-surface-2 hover:text-ink"
              }`}
            >
              <Icon size={20} strokeWidth={active ? 2.2 : 1.8} />
              {isChatStreaming && (
                <span className="absolute right-2 top-2 h-2 w-2 rounded-full bg-copper-hover animate-pulse" />
              )}
              {active && (
                <span className="absolute left-0 top-1/2 h-6 w-1 -translate-y-1/2 rounded-r-full bg-copper-hover" />
              )}
            </button>
          );
        })}
      </div>

      <div className="flex-1" />

      <button
        className="icon-btn h-9 w-9 text-dim hover:text-ink"
        onClick={toggleActivityBar}
        title="Collapse sidebar"
        aria-label="Collapse sidebar"
      >
        <PanelLeftClose size={18} />
      </button>
    </nav>
  );
}
