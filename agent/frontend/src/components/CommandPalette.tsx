import { useEffect, useMemo, useRef, useState } from "react";
import {
  Blocks,
  CornerDownLeft,
  FileCode2,
  FileText,
  MessageSquare,
  Settings,
  TerminalSquare,
  Wrench,
} from "lucide-react";

import { useStore } from "../store";

interface Entry {
  id: string;
  kind: "command" | "file" | "provider" | "agent" | "view";
  title: string;
  hint?: string;
  icon: React.ReactNode;
  run: () => void;
}

function score(q: string, text: string): number {
  const needle = q.toLowerCase();
  const hay = text.toLowerCase();
  let i = 0;
  for (const ch of needle) {
    const idx = hay.indexOf(ch, i);
    if (idx === -1) return 0;
    i = idx + 1;
  }
  return 1;
}

/** Ctrl+K command palette with fuzzy search over everything. */
export default function CommandPalette() {
  const open = useStore((s) => s.paletteOpen);
  const setOpen = useStore((s) => s.setPaletteOpen);
  const s = useStore.getState;

  const [q, setQ] = useState("");
  const [idx, setIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQ("");
      setIdx(0);
      window.setTimeout(() => inputRef.current?.focus(), 10);
    }
  }, [open]);

  const entries = useMemo<Entry[]>(() => {
    const st = s();
    const list: Entry[] = [
      {
        id: "cmd-new-chat",
        kind: "command",
        title: "New chat message",
        icon: <MessageSquare size={14} className="text-copper" />,
        run: () => {
          st.setView("chat");
          setOpen(false);
        },
      },
      {
        id: "cmd-picker",
        kind: "command",
        title: "Switch provider / model",
        hint: "F6",
        icon: <Wrench size={14} className="text-copper" />,
        run: () => {
          st.setProviderPickerOpen(true);
          setOpen(false);
        },
      },
      {
        id: "cmd-tui",
        kind: "command",
        title: "Open TUI client",
        hint: "Ctrl+Shift+X",
        icon: <TerminalSquare size={14} className="text-copper" />,
        run: () => {
          st.spawnTui();
          setOpen(false);
        },
      },
      {
        id: "cmd-stats",
        kind: "command",
        title: "Refresh system stats",
        icon: <Wrench size={14} className="text-copper" />,
        run: () => {
          st.refreshStats();
          setOpen(false);
        },
      },
    ];
    for (const p of st.providers) {
      list.push({
        id: `prov-${p.name}`,
        kind: "provider",
        title: p.name,
        hint: p.has_key ? "connected" : p.needs_key ? "needs key" : "local",
        icon: <Wrench size={14} className="text-copper" />,
        run: () => {
          st.switchProvider(p.name);
          st.requestModels(p.name);
          setOpen(false);
        },
      });
    }
    for (const a of st.agents) {
      list.push({
        id: `agent-${a}`,
        kind: "agent",
        title: `Delegate to ${a}`,
        hint: "external agent",
        icon: <Blocks size={14} className="text-copper" />,
        run: () => {
          st.setView("agents");
          setOpen(false);
        },
      });
    }
    for (const tab of st.openTabs) {
      list.push({
        id: `tab-${tab.path}`,
        kind: "file",
        title: tab.path,
        icon: <FileCode2 size={14} className="text-copper" />,
        run: () => {
          st.setView("editor");
          st.openTab(tab.path);
          setOpen(false);
        },
      });
    }
    const views: { id: string; title: string; icon: React.ReactNode }[] = [
      { id: "v-chat", title: "Go to Chat", icon: <MessageSquare size={14} className="text-copper" /> },
      { id: "v-explorer", title: "Go to Explorer", icon: <FileText size={14} className="text-copper" /> },
      { id: "v-editor", title: "Go to Editor", icon: <FileCode2 size={14} className="text-copper" /> },
      { id: "v-terminal", title: "Go to Terminal", icon: <TerminalSquare size={14} className="text-copper" /> },
      { id: "v-agents", title: "Go to Agents", icon: <Blocks size={14} className="text-copper" /> },
      { id: "v-settings", title: "Go to Settings", icon: <Settings size={14} className="text-copper" /> },
    ];
    for (const v of views) {
      list.push({
        id: v.id,
        kind: "view",
        title: v.title,
        icon: v.icon,
        run: () => {
          st.setView(v.id.replace("v-", "") as never);
          setOpen(false);
        },
      });
    }
    return list;
  }, [s, setOpen]);

  const filtered = useMemo(
    () =>
      q.trim()
        ? entries.filter((e) => score(q, `${e.title} ${e.kind} ${e.hint ?? ""}`) > 0)
        : entries,
    [entries, q]
  );

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/55 pt-[16vh] backdrop-blur-[2px]"
      onMouseDown={() => setOpen(false)}
    >
      <div
        className="glass w-[min(560px,92vw)] overflow-hidden rounded-panel shadow-2xl"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-border-subtle px-3 py-2.5">
          <span className="text-[12px] font-semibold uppercase tracking-widest text-copper">Palette</span>
          <input
            ref={inputRef}
            value={q}
            onChange={(e) => {
              setQ(e.target.value);
              setIdx(0);
            }}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setIdx((i) => Math.min(i + 1, filtered.length - 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setIdx((i) => Math.max(i - 1, 0));
              } else if (e.key === "Enter" && filtered[idx]) {
                filtered[idx].run();
              } else if (e.key === "Escape") {
                setOpen(false);
              }
            }}
            placeholder="Type a command, provider, file, or view…"
            className="flex-1 bg-transparent px-1 text-[13.5px] text-ink placeholder:text-faint outline-none"
          />
        </div>
        <div className="max-h-[300px] overflow-y-auto py-1.5">
          {filtered.map((e, i) => (
            <button
              key={e.id}
              onClick={e.run}
              onMouseEnter={() => setIdx(i)}
              className={`flex w-full items-center gap-2.5 px-3 py-1.5 text-left transition-colors ${
                i === idx ? "bg-copper/15" : ""
              }`}
            >
              <span className="shrink-0">{e.icon}</span>
              <span className="min-w-0 flex-1 truncate text-[13px] text-ink">{e.title}</span>
              {e.hint && (
                <span className="shrink-0 rounded bg-surface-2 px-1.5 py-0.5 text-[10px] text-dim">
                  {e.hint}
                </span>
              )}
              {i === idx && <CornerDownLeft size={12} className="shrink-0 text-copper" />}
            </button>
          ))}
          {!filtered.length && (
            <div className="px-3 py-6 text-center text-[12px] text-faint">
              No matches for “{q}”
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
