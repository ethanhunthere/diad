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
 * Clean, pro 56px activity bar with large accessible targets and industrial basalt styling.
 */
export function ActivityBar({ collapsed }: { collapsed: boolean }) {
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);
  const toggleActivityBar = useStore((s) => s.toggleActivityBar);

  if (collapsed) {
    return (
      <nav className="activity-bar flex w-0 shrink-0 flex-col items-center border-r border-[#202623] bg-[#0F1110] py-3 transition-all duration-150 ease-out">
        <button
          className="icon-btn h-9 w-9 text-[#8A968E] hover:text-[#E5EAE6]"
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
    <nav className="activity-bar flex w-14 shrink-0 flex-col items-center border-r border-[#202623] bg-[#0F1110] py-3 transition-all duration-150 ease-out select-none">
      <div className="flex flex-col gap-2.5">
        {PRIMARY_VIEWS.map(({ id, label, icon: Icon }) => {
          const active = view === id;
          return (
            <button
              key={id}
              onClick={() => setView(id)}
              onMouseEnter={() => prefetchView(id)}
              title={label}
              aria-label={label}
              className={`relative flex h-11 w-11 items-center justify-center rounded-xl transition-all duration-150 ${
                active
                  ? "bg-[#B32032]/20 text-[#E5EAE6] shadow-sm ring-1 ring-[#B32032]/60"
                  : "text-[#8A968E] hover:bg-[#161918] hover:text-[#E5EAE6]"
              }`}
            >
              <Icon size={20} />
              {active && (
                <span className="absolute -left-1 h-5 w-1 rounded-r-full bg-[#B32032]" />
              )}
            </button>
          );
        })}
      </div>

      <div className="flex-1" />

      <button
        className="flex h-9 w-9 items-center justify-center rounded-lg text-[#525D56] hover:bg-[#161918] hover:text-[#8A968E] transition-colors"
        onClick={toggleActivityBar}
        title="Collapse sidebar"
        aria-label="Collapse sidebar"
      >
        <PanelLeftClose size={16} />
      </button>
    </nav>
  );
}
