import { useEffect, useMemo, useRef, useState } from "react";
import {
  Search,
  MessageSquare,
  FolderOpen,
  FileCode2,
  TerminalSquare,
  Bot,
  Settings2,
  Wrench,
  KeyRound,
  ArrowRight,
} from "lucide-react";
import { useStore, sendMsg } from "../store/useStore";
import { nextId } from "../lib/format";

interface PaletteItem {
  id: string;
  label: string;
  hint?: string;
  icon: typeof Search;
  run: () => void;
}

/**
 * Command palette — Ctrl+K fuzzy search over views, commands,
 * providers and tools.
 */
export function CommandPalette() {
  const setPaletteOpen = useStore((s) => s.setPaletteOpen);
  const setView = useStore((s) => s.setView);
  const providers = useStore((s) => s.providers);
  const setPendingConnectProvider = useStore((s) => s.setPendingConnectProvider);
  const [query, setQuery] = useState("");
  const [idx, setIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const items = useMemo<PaletteItem[]>(() => {
    const base: PaletteItem[] = [
      { id: "v-chat", label: "Go to Chat", icon: MessageSquare, run: () => setView("chat") },
      { id: "v-explorer", label: "Go to Explorer", icon: FolderOpen, run: () => setView("explorer") },
      { id: "v-editor", label: "Go to Editor", icon: FileCode2, run: () => setView("editor") },
      { id: "v-terminal", label: "Go to Terminal", icon: TerminalSquare, run: () => setView("terminal") },
      { id: "v-agents", label: "Go to Agents", icon: Bot, run: () => setView("agents") },
      { id: "v-settings", label: "Go to Settings", icon: Settings2, run: () => setView("settings") },
      { id: "c-clear", label: "Clear chat", icon: Wrench, run: () => useStore.setState({ messages: [] }) },
      { id: "c-spawn-tui", label: "Open TUI client", icon: TerminalSquare, run: () => sendMsg({ type: "spawn.tui" }) },
    ];
    const providerItems: PaletteItem[] = providers.map((p) => ({
      id: `p-${p.name}`,
      label: p.has_key ? `Switch provider → ${p.name}` : `Connect provider → ${p.name}`,
      hint: p.has_key ? "switch" : "needs key",
      icon: KeyRound,
      run: () => {
        if (p.has_key) {
          sendMsg({ type: "provider.switch", name: p.name });
        } else {
          setPendingConnectProvider(p.name);
        }
      },
    }));
    const slashItems: PaletteItem[] = ["/help", "/tools", "/version", "/model", "/wifi"].map((c) => ({
      id: `s-${c}`,
      label: c,
      hint: "send command",
      icon: ArrowRight,
      run: () => {
        sendMsg({ type: "chat.send", id: nextId("pal"), text: c });
        setView("chat");
      },
    }));
    return [...base, ...providerItems, ...slashItems];
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [providers, setView, setPaletteOpen, setPendingConnectProvider]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return items;
    return items.filter(
      (i) =>
        i.label.toLowerCase().includes(q) ||
        (i.hint ?? "").toLowerCase().includes(q),
    );
  }, [items, query]);

  useEffect(() => setIdx(0), [query]);

  const close = () => setPaletteOpen(false);

  const run = (item: PaletteItem) => {
    item.run();
    close();
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") close();
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setIdx((i) => Math.min(i + 1, filtered.length - 1));
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setIdx((i) => Math.max(i - 1, 0));
    }
    if (e.key === "Enter" && filtered[idx]) {
      e.preventDefault();
      run(filtered[idx]);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-[16vh] backdrop-blur-sm animate-fade-in"
      onMouseDown={close}
    >
      <div
        className="glass w-[560px] overflow-hidden rounded-panel shadow-2xl"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2.5 border-b border-border-subtle px-3.5 py-3">
          <Search size={15} className="text-faint" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKey}
            placeholder="Type a command, provider or /command…"
            className="flex-1 bg-transparent text-[13.5px] text-ink placeholder:text-faint focus:outline-none"
          />
          <span className="kbd">Esc</span>
        </div>

        <div className="max-h-[46vh] overflow-y-auto py-1.5">
          {filtered.length === 0 && (
            <div className="px-4 py-6 text-center text-[12px] text-faint">
              No matches for “{query}”
            </div>
          )}
          {filtered.map((item, i) => (
            <button
              key={item.id}
              onMouseEnter={() => setIdx(i)}
              onClick={() => run(item)}
              className={`flex w-full items-center gap-2.5 px-3.5 py-2 text-left text-[13px] ${
                i === idx ? "bg-copper/10 text-ink" : "text-dim"
              }`}
            >
              <item.icon size={14} className={i === idx ? "text-copper-hover" : "text-faint"} />
              <span className="flex-1">{item.label}</span>
              {item.hint && <span className="text-[10px] text-faint">{item.hint}</span>}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
