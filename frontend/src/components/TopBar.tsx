import { useEffect, useState } from "react";
import { useStore } from "../stores/useStore";
import { clock12 } from "../lib/format";

/**
 * Top bar — wordmark, palette trigger, provider·model pill,
 * connection dot, GUI⇄TUI toggle, clock.
 */
export function TopBar() {
  const provider = useStore((s) => s.provider);
  const model = useStore((s) => s.model);
  const connected = useStore((s) => s.connected);
  const wsConnected = useStore((s) => s.wsConnected);
  const daemonVersion = useStore((s) => s.daemonVersion);
  const setPaletteOpen = useStore((s) => s.setPaletteOpen);
  const toast = useStore((s) => s.toast);
  const send = useStore((s) => s.send);

  const online = wsConnected && connected;

  // Live clock.
  const [time, setTime] = useState(clock12());
  useEffect(() => {
    const t = setInterval(() => setTime(clock12()), 1000);
    return () => clearInterval(t);
  }, []);

  const spawnTui = () => {
    if (send) send({ v: 1, msg: { type: "spawn.tui" } });
    else toast({ kind: "warn", title: "Daemon offline", message: "Cannot open TUI while the daemon is disconnected.", ttl: 4000 });
  };

  return (
    <header className="glass z-30 flex h-11 shrink-0 items-center gap-3 border-b border-border-subtle px-3">
      {/* Wordmark */}
      <div className="flex items-center gap-2 select-none">
        <span className="flex h-5 w-5 items-center justify-center rounded-[6px] border border-copper/40 bg-copper/15 text-[11px] font-bold text-copper-hover">
          ◈
        </span>
        <span className="text-[13px] font-semibold tracking-[0.08em] text-ink">
          DIAD <span className="text-copper-hover">AGENT</span>
        </span>
        {daemonVersion && (
          <span className="font-mono text-[10px] text-faint">v{daemonVersion}</span>
        )}
      </div>

      <div className="mx-1 h-4 w-px bg-border-strong" />

      {/* Palette trigger */}
      <button
        className="btn-ghost h-7 gap-2 px-2 text-dim hover:text-ink"
        onClick={() => setPaletteOpen(true)}
        title="Command palette (Ctrl+K)"
      >
        <span className="text-[12px]">⌘</span>
        <span className="text-[12px]">Quick commands…</span>
        <span className="kbd">Ctrl K</span>
      </button>

      <div className="flex-1" />

      {/* Provider · model pill */}
      <div
        className="flex items-center gap-2 rounded-full border border-border-subtle bg-surface-2/60 px-3 py-1 text-[11px]"
        title={`Provider: ${provider || "none"} · Model: ${model || "none"}`}
      >
        <span className="text-dim">provider</span>
        <span className="font-medium text-ink">{provider || "—"}</span>
        <span className="text-faint">·</span>
        <span className="text-dim">model</span>
        <span className="max-w-[160px] truncate font-medium text-ink">{model || "—"}</span>
      </div>

      {/* Connection dot */}
      <div
        className={`flex items-center gap-1.5 text-[11px] ${online ? "text-success" : "text-warn"}`}
        title={online ? "ONLINE" : "OFFLINE"}
      >
        <span
          className={`h-2 w-2 rounded-full ${
            online
              ? "bg-success shadow-[0_0_6px_rgba(111,191,139,0.8)]"
              : "bg-warn shadow-[0_0_6px_rgba(224,176,96,0.8)] animate-pulse-dot"
          }`}
        />
        <span className="font-medium tracking-wide">{online ? "ONLINE" : "OFFLINE"}</span>
      </div>

      {/* GUI ⇄ TUI toggle */}
      <button
        className="btn-ghost h-7 gap-1.5 px-2 text-[11px] text-dim hover:text-ink"
        onClick={spawnTui}
        title="Open the TUI client (Ctrl+Shift+X)"
      >
        <span className="font-mono text-[12px]">▮▯</span>
        TUI
      </button>

      <div className="mx-1 h-4 w-px bg-border-strong" />

      {/* Clock */}
      <div className="w-[76px] text-right font-mono text-[11px] text-dim select-none">
        {time}
      </div>
    </header>
  );
}
