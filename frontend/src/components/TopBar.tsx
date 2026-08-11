import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useStore, sendMsg } from "../stores/useStore";
import { clock12, nextId } from "../lib/format";

/**
 * Top bar — wordmark, palette trigger, provider·model pill,
 * connection dot, GUI⇄TUI toggle, power menu, clock. Accepts an
 * optional hamburger slot, shown only at the TINY breakpoint (see CSS).
 */
export function TopBar({ hamburger }: { hamburger?: ReactNode }) {
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

  // Power menu (RBRO-004 WP-06.6 — GUI ⇄ TUI parity: the same
  // daemon slash commands the TUI accepts, always confirmed).
  const [powerOpen, setPowerOpen] = useState(false);
  const powerRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!powerOpen) return;
    const onDoc = (e: MouseEvent) => {
      if (powerRef.current && !powerRef.current.contains(e.target as Node)) {
        setPowerOpen(false);
      }
    };
    const onEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") setPowerOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onEsc);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onEsc);
    };
  }, [powerOpen]);

  const powerAction = (action: "sleep" | "reboot" | "shutdown") => {
    setPowerOpen(false);
    if (!send) {
      toast({ kind: "warn", title: "Daemon offline", message: "Cannot send power command while the daemon is disconnected.", ttl: 4000 });
      return;
    }
    sendMsg({ type: "chat_send", id: nextId("power"), text: `/${action}` });
  };

  const spawnTui = () => {
    if (send) send({ v: 1, type: "spawn_tui" });
    else toast({ kind: "warn", title: "Daemon offline", message: "Cannot open TUI while the daemon is disconnected.", ttl: 4000 });
  };

  return (
    <header className="glass z-30 flex h-11 shrink-0 items-center gap-3 border-b border-border-subtle px-3">
      {/* TINY-breakpoint menu button (hidden otherwise, see index.css) */}
      {hamburger}

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

      {/* Provider · model pill (hidden on TINY, see index.css) */}
      <div
        className="topbar-pill flex items-center gap-2 rounded-full border border-border-subtle bg-surface-2/60 px-3 py-1 text-[11px]"
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

      {/* Power menu — Sleep / Restart / Shutdown (confirmed daemon-side) */}
      <div className="relative" ref={powerRef}>
        <button
          className={`btn-ghost h-7 px-2 text-[12px] ${powerOpen ? "text-copper-hover" : "text-dim hover:text-ink"}`}
          onClick={() => setPowerOpen((o) => !o)}
          title="Power (Sleep / Restart / Shutdown)"
          aria-expanded={powerOpen}
          aria-haspopup="menu"
        >
          ⏻
        </button>
        {powerOpen && (
          <div
            role="menu"
            className="glass absolute right-0 top-9 z-40 w-40 overflow-hidden rounded-lg border border-border-subtle bg-surface-2/95 py-1 shadow-lg"
          >
            <button
              role="menuitem"
              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-[12px] text-dim hover:bg-surface-3/80 hover:text-ink"
              onClick={() => powerAction("sleep")}
            >
              <span className="w-4 text-center">☾</span> Sleep
            </button>
            <button
              role="menuitem"
              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-[12px] text-dim hover:bg-surface-3/80 hover:text-ink"
              onClick={() => powerAction("reboot")}
            >
              <span className="w-4 text-center">⟳</span> Restart
            </button>
            <div className="my-1 h-px bg-border-strong" />
            <button
              role="menuitem"
              className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-[12px] text-danger hover:bg-danger/10 hover:text-danger"
              onClick={() => powerAction("shutdown")}
            >
              <span className="w-4 text-center">⏻</span> Shutdown
            </button>
          </div>
        )}
      </div>

      <div className="mx-1 h-4 w-px bg-border-strong" />

      {/* Clock */}
      <div className="w-[76px] text-right font-mono text-[11px] text-dim select-none">
        {time}
      </div>
    </header>
  );
}
