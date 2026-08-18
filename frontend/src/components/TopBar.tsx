import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useStore, sendMsg } from "../stores/useStore";
import { DiadLogo } from "./DiadLogo";

/**
 * TopBar — clean, compact studio header with model selector, command palette trigger,
 * connection status, TUI switcher, and window controls.
 */
export function TopBar({ hamburger }: { hamburger?: ReactNode }) {
  const provider = useStore((s) => s.provider);
  const model = useStore((s) => s.model);
  const providers = useStore((s) => s.providers);
  const models = useStore((s) => s.models);
  const setView = useStore((s) => s.setView);
  const connected = useStore((s) => s.connected);
  const wsConnected = useStore((s) => s.wsConnected);
  const setPaletteOpen = useStore((s) => s.setPaletteOpen);
  const toast = useStore((s) => s.toast);
  const send = useStore((s) => s.send);

  const online = wsConnected && connected;

  const [providerOpen, setProviderOpen] = useState(false);
  const providerRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!providerOpen) return;
    const onDoc = (e: MouseEvent) => {
      if (providerRef.current && !providerRef.current.contains(e.target as Node)) {
        setProviderOpen(false);
      }
    };
    const onEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") setProviderOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onEsc);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onEsc);
    };
  }, [providerOpen]);

  const selectProvider = (name: string) => {
    sendMsg({ type: "provider_switch", name });
    sendMsg({ type: "config_set", key: "default_provider", value: name });
    toast({ kind: "info", title: "Provider Active", message: name, ttl: 2000 });
  };

  const selectModel = (m: string) => {
    sendMsg({ type: "model_switch", model: m });
    sendMsg({ type: "config_set", key: "default_model", value: m });
    setProviderOpen(false);
    toast({ kind: "info", title: "Model Selected", message: m, ttl: 2000 });
  };

  const spawnTui = async () => {
    if (send) send({ v: 1, type: "spawn_tui" });
    else toast({ kind: "warn", title: "Daemon offline", message: "Cannot open TUI while disconnected.", ttl: 3000 });
  };

  const windowAction = async (action: "minimize" | "maximize" | "close") => {
    try {
      await fetch("/api/v1/window", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ action }),
      });
    } catch {}
  };

  return (
    <header className="glass z-30 flex h-10 shrink-0 items-center gap-2.5 border-b border-border-subtle/80 px-3.5 select-none bg-[#12141d]">
      {hamburger}

      {/* DIAD Wordmark */}
      <DiadLogo size={14} />

      <div className="mx-1.5 h-3.5 w-px bg-border-strong hidden sm:block" />

      {/* Quick search / command trigger */}
      <button
        className="flex h-7 items-center gap-1.5 rounded-lg border border-border-subtle bg-surface-2/70 px-2.5 text-[12px] text-dim hover:border-amber-500/50 hover:text-ink transition-colors"
        onClick={() => setPaletteOpen(true)}
        title="Open command palette (Ctrl+K)"
      >
        <span>⌘</span>
        <span className="hidden md:inline">Search or command…</span>
        <span className="kbd text-[10px] ml-1 hidden sm:inline">Ctrl K</span>
      </button>

      <div className="flex-1" />

      {/* Provider & Model Selector */}
      <div className="relative" ref={providerRef}>
        <button
          className={`flex h-7 items-center gap-1.5 rounded-lg border px-2.5 text-[12px] font-medium transition-all ${
            providerOpen
              ? "border-amber-500/60 bg-amber-500/20 text-ink shadow-sm"
              : "border-border-subtle bg-surface-2/70 text-dim hover:border-amber-500/40 hover:text-ink"
          }`}
          onClick={() => setProviderOpen((o) => !o)}
          title="Change AI model (F6)"
          aria-expanded={providerOpen}
          aria-haspopup="menu"
        >
          <span className="h-1.5 w-1.5 rounded-full bg-amber-400" />
          <span className="text-ink font-semibold">{provider || "AI"}</span>
          <span className="text-faint">/</span>
          <span className="max-w-[130px] truncate text-amber-400 font-semibold">{model || "Select"}</span>
          <span className="text-[10px] text-faint">▾</span>
        </button>

        {providerOpen && (
          <div
            role="menu"
            className="glass absolute right-0 top-9 z-50 w-80 overflow-hidden rounded-xl border border-amber-500/40 bg-[#161824] p-2.5 shadow-2xl backdrop-blur-xl animate-fade-in"
          >
            <div className="flex items-center justify-between border-b border-border-subtle/80 pb-2 px-2">
              <span className="text-[11.5px] font-semibold uppercase tracking-wider text-dim">
                Select Model
              </span>
              <button
                className="text-[11.5px] font-medium text-amber-400 hover:underline"
                onClick={() => {
                  setProviderOpen(false);
                  setView("settings");
                }}
              >
                API Keys ⚙
              </button>
            </div>

            <div className="mt-2 max-h-72 overflow-y-auto space-y-1.5 pr-1">
              {providers.length === 0 && (
                <div className="px-3 py-4 text-center text-[13px] text-dim">
                  No providers configured.
                  <button
                    className="mt-2 block w-full text-center text-xs font-semibold text-amber-400 hover:underline"
                    onClick={() => {
                      setProviderOpen(false);
                      setView("settings");
                    }}
                  >
                    Add API Key in Settings →
                  </button>
                </div>
              )}
              {providers.map((p) => {
                const isCurrentProvider = p.name === provider;
                return (
                  <div
                    key={p.name}
                    className={`rounded-lg border p-2 text-left transition-all ${
                      isCurrentProvider
                        ? "border-amber-500/50 bg-amber-500/10"
                        : "border-transparent hover:border-border-subtle hover:bg-surface-3/60"
                    }`}
                  >
                    <button
                      className="flex w-full items-center gap-2 text-[12.5px] font-semibold text-ink capitalize"
                      onClick={() => selectProvider(p.name)}
                    >
                      <span
                        className={`h-2 w-2 rounded-full ${
                          p.has_key ? "bg-emerald-400" : "bg-faint"
                        }`}
                      />
                      {p.name}
                    </button>

                    {isCurrentProvider && models.length > 0 && (
                      <div className="mt-1.5 flex flex-wrap gap-1">
                        {models.slice(0, 6).map((m) => (
                          <button
                            key={m}
                            className={`rounded-md px-2 py-0.5 text-[11px] font-medium transition-colors ${
                              model === m
                                ? "bg-amber-500 text-slate-950 font-bold shadow-sm"
                                : "bg-surface-3 text-dim hover:text-ink hover:bg-surface-3/90"
                            }`}
                            onClick={() => selectModel(m)}
                          >
                            {m}
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>

            <div className="mt-2 border-t border-border-subtle/80 pt-2 px-1">
              <button
                className="flex w-full items-center justify-center gap-1.5 rounded-lg border border-border-subtle bg-surface-3/80 py-1.5 text-center text-[11.5px] font-medium text-ink hover:border-amber-500/50 hover:text-amber-300 transition-colors"
                onClick={() => {
                  setProviderOpen(false);
                  setView("settings");
                }}
              >
                ⚙ Open Settings & API Keys
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Terminal / TUI Switcher */}
      <button
        className="flex h-7 items-center gap-1 rounded-lg border border-border-subtle bg-surface-2/70 px-2 text-[11.5px] font-medium text-dim hover:border-amber-500/40 hover:text-ink transition-colors"
        onClick={spawnTui}
        title="Open TUI terminal mode (F5)"
      >
        <span className="font-mono text-amber-400 font-bold">▮</span>
        <span>TUI</span>
      </button>

      {/* Connection indicator */}
      <div
        className={`flex items-center gap-1.5 text-[11.5px] font-medium hidden sm:flex ${online ? "text-dim" : "text-amber-400"}`}
        title={online ? "Online" : "Disconnected"}
      >
        <span
          className={`h-1.5 w-1.5 rounded-full ${
            online ? "bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.8)]" : "bg-amber-400"
          }`}
        />
        <span>{online ? "Online" : "Offline"}</span>
      </div>

      <div className="mx-1 h-3.5 w-px bg-border-strong" />

      {/* Window controls [–] [□] [×] */}
      <div className="flex items-center gap-0.5 rounded-lg border border-border-subtle bg-surface-2/70 p-0.5">
        <button
          className="flex h-6 w-6 items-center justify-center rounded text-[13px] font-medium text-dim hover:bg-surface-3 hover:text-ink transition-colors"
          onClick={() => windowAction("minimize")}
          title="Minimize"
          aria-label="Minimize"
        >
          –
        </button>
        <button
          className="flex h-6 w-6 items-center justify-center rounded text-[11px] font-medium text-dim hover:bg-surface-3 hover:text-ink transition-colors"
          onClick={() => windowAction("maximize")}
          title="Maximize"
          aria-label="Maximize"
        >
          □
        </button>
        <button
          className="flex h-6 w-6 items-center justify-center rounded text-[13px] font-medium text-dim hover:bg-red-500/25 hover:text-red-400 transition-colors"
          onClick={() => windowAction("close")}
          title="Close"
          aria-label="Close"
        >
          ×
        </button>
      </div>
    </header>
  );
}
