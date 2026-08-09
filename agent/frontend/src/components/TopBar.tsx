import { useEffect, useState } from "react";
import {
  ChevronDown,
  Command,
  Loader2,
  SquareTerminal,
} from "lucide-react";

import { useStore } from "../store";

function Clock() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const t = window.setInterval(() => setNow(new Date()), 1000);
    return () => window.clearInterval(t);
  }, []);
  return (
    <span className="font-mono text-[12px] tabular-nums text-dim">
      {now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
    </span>
  );
}

/** Top command bar: wordmark · palette hint · provider/model pill · status · clock. */
export default function TopBar() {
  const provider = useStore((s) => s.provider);
  const model = useStore((s) => s.model);
  const connected = useStore((s) => s.connected);
  const ws = useStore((s) => s.ws);
  const streaming = useStore((s) => s.streaming);
  const openPalette = useStore((s) => s.setPaletteOpen);
  const openPicker = useStore((s) => s.setProviderPickerOpen);
  const spawnTui = useStore((s) => s.spawnTui);

  const dot =
    ws === "connected"
      ? connected
        ? "bg-success"
        : "bg-warn"
      : "bg-danger";

  return (
    <header className="glass relative z-30 flex h-11 shrink-0 items-center gap-3 border-b border-border-subtle px-3">
      {/* Wordmark */}
      <div className="flex items-center gap-2">
        <span className="flex h-6 w-6 items-center justify-center rounded-md bg-copper font-mono text-[13px] font-bold text-[#1a0f07]">
          D
        </span>
        <span className="text-[13px] font-bold tracking-[0.18em] text-ink">
          DIAD<span className="text-copper"> AGENT</span>
        </span>
      </div>

      {/* Palette hint */}
      <button
        onClick={() => openPalette(true)}
        className="hidden items-center gap-2 rounded-control border border-border-subtle bg-obsidian/40 px-2.5 py-1 text-[12px] text-faint transition-colors hover:border-border-strong hover:text-dim md:flex"
        title="Command palette (Ctrl+K)"
      >
        <Command size={12} />
        <span>Search commands, files, providers…</span>
        <kbd className="rounded border border-border-subtle bg-surface-2 px-1 font-mono text-[10px] text-dim">
          Ctrl K
        </kbd>
      </button>

      <div className="flex-1" />

      {/* Streaming indicator */}
      {streaming && (
        <span className="flex items-center gap-1.5 text-[12px] text-copper">
          <Loader2 size={13} className="animate-spin" />
          thinking…
        </span>
      )}

      {/* Provider · model pill */}
      <button
        onClick={() => openPicker(true)}
        className="group flex items-center gap-2 rounded-control border border-border-subtle bg-surface px-2.5 py-1 transition-colors hover:border-copper/50"
        title="Switch provider / model (F6)"
      >
        <span className="flex items-center gap-1.5">
          <span className={`h-1.5 w-1.5 rounded-full ${dot}`} />
          <span className="text-[12px] font-semibold text-ink">{provider}</span>
          <span className="font-mono text-[11px] text-dim">{model || "—"}</span>
        </span>
        <ChevronDown size={12} className="text-faint transition-colors group-hover:text-copper" />
      </button>

      {/* TUI toggle */}
      <button
        onClick={spawnTui}
        className="flex items-center gap-1.5 rounded-control border border-border-subtle bg-surface px-2.5 py-1 text-[12px] text-dim transition-colors hover:border-copper/50 hover:text-ink"
        title="Open TUI client (Ctrl+Shift+X)"
      >
        <SquareTerminal size={13} />
        <span className="hidden lg:inline">TUI</span>
      </button>

      <Clock />
    </header>
  );
}
