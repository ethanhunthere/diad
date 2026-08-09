import {
  Blocks,
  FileCode2,
  Files,
  MessageSquare,
  Settings,
  TerminalSquare,
} from "lucide-react";

import { useStore } from "../store";
import { View } from "../lib/protocol";

const ITEMS: { view: View; icon: typeof MessageSquare; label: string }[] = [
  { view: "chat", icon: MessageSquare, label: "Chat" },
  { view: "explorer", icon: Files, label: "Explorer" },
  { view: "editor", icon: FileCode2, label: "Editor" },
  { view: "terminal", icon: TerminalSquare, label: "Terminal" },
  { view: "agents", icon: Blocks, label: "Agents" },
  { view: "settings", icon: Settings, label: "Settings" },
];

/** 48px vertical rail with view switching (Ctrl+1..6). */
export default function ActivityBar() {
  const view = useStore((s) => s.view);
  const setView = useStore((s) => s.setView);

  return (
    <nav className="glass z-20 flex w-12 shrink-0 flex-col items-center gap-1 border-r border-border-subtle py-2">
      {ITEMS.map(({ view: v, icon: Icon, label }, i) => (
        <button
          key={v}
          onClick={() => setView(v)}
          title={`${label} (Ctrl+${i + 1})`}
          className={`relative flex h-9 w-9 items-center justify-center rounded-control transition-colors duration-100 ${
            view === v
              ? "bg-copper/15 text-copper"
              : "text-faint hover:bg-surface-2 hover:text-ink"
          }`}
        >
          {view === v && (
            <span className="absolute -left-2.5 top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-r-full bg-copper" />
          )}
          <Icon size={19} strokeWidth={1.8} />
        </button>
      ))}
    </nav>
  );
}
