import {
  MessageSquare,
  FolderOpen,
  FileCode2,
  TerminalSquare,
  Bot,
  Settings2,
  GitBranch,
  Bug,
  Eye,
  FolderKanban,
  PanelLeftClose,
  PanelLeftOpen,
  ClipboardCopy,
  Trash2,
} from "lucide-react";
import { useStore } from "../stores/useStore";
import type { ViewId } from "../lib/types";
import { VIEW_LABELS } from "../lib/format";
import { prefetchView } from "../lib/prefetch";

export const VIEWS: { id: ViewId; icon: typeof MessageSquare }[] = [
  { id: "chat", icon: MessageSquare },
  { id: "explorer", icon: FolderOpen },
  { id: "editor", icon: FileCode2 },
  { id: "git", icon: GitBranch },
  { id: "debug", icon: Bug },
  { id: "preview", icon: Eye },
  { id: "projects", icon: FolderKanban },
  { id: "terminal", icon: TerminalSquare },
  { id: "agents", icon: Bot },
  { id: "clipboard", icon: ClipboardCopy },
  { id: "trash", icon: Trash2 },
  { id: "settings", icon: Settings2 },
];

/**
 * 48px left activity bar with view switcher and a collapse toggle.
 */
export function ActivityBar({ collapsed }: { collapsed: boolean }) {
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);
  const toggleActivityBar = useStore((s) => s.toggleActivityBar);
  const openFiles = useStore((s) => s.openFiles);
  const streaming = useStore((s) => s.streaming);

  if (collapsed) {
    return (
      <nav className="activity-bar flex w-0 shrink-0 flex-col items-center border-r border-border-subtle bg-surface/40 py-2 transition-all duration-150 ease-out">
        <button className="icon-btn" onClick={toggleActivityBar} title="Expand activity bar (Ctrl+B)">
          <PanelLeftOpen size={16} />
        </button>
      </nav>
    );
  }

  return (
    <nav className="activity-bar flex w-12 shrink-0 flex-col items-center border-r border-border-subtle bg-surface/40 py-2 transition-all duration-150 ease-out">
      {VIEWS.map(({ id, icon: Icon }) => {
        const active = view === id;
        const badge =
          id === "editor" && openFiles.some((f) => f.dirty)
            ? "•"
            : id === "chat" && streaming
              ? "●"
              : null;
        return (
          <button
            key={id}
            onClick={() => setView(id)}
            onMouseEnter={() => prefetchView(id)}
            title={VIEW_LABELS[id]}
            className={`relative flex h-10 w-10 items-center justify-center rounded-panel transition-all duration-150 ease-out ${
              active
                ? "bg-copper/15 text-copper-hover"
                : "text-faint hover:bg-surface-2 hover:text-dim"
            }`}
          >
            <Icon size={18} strokeWidth={active ? 2.2 : 1.8} />
            {badge && (
              <span className="absolute right-1.5 top-1.5 text-[10px] text-copper-hover">
                {badge}
              </span>
            )}
            {active && (
              <span className="absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-full bg-copper" />
            )}
          </button>
        );
      })}

      <div className="flex-1" />

      <button
        className="icon-btn"
        onClick={toggleActivityBar}
        title="Collapse activity bar (Ctrl+B)"
      >
        <PanelLeftClose size={16} />
      </button>
    </nav>
  );
}
