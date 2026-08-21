import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { useStore, sendMsg } from "../stores/useStore";
import { DiadLogo } from "./DiadLogo";

/**
 * TopBar — clean, spacious studio header with model selector, command palette trigger,
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
    <header className="glass z-30 flex h-12 shrink-0 items-center gap-2 sm:gap-3 border-b border-border-subtle px-3 sm:px-4 select-none bg-surface">
      {hamburger}

      {/* DIAD Wordmark */}
      <DiadLogo size={16} />

      <div className="mx-1 h-4 w-px bg-border-subtle hidden sm:block" />

      {/* Quick search / command trigger */}
      <button
        className="flex h-8 items-center gap-2 rounded-lg border border-border-subtle bg-surface-2 px-2.5 sm:px-3 text-[12px] sm:text-[13px] text-dim hover:border-copper hover:text-ink transition-colors"
        onClick={() => setPaletteOpen(true)}
        title="Open command palette (Ctrl+K)"
      >
        <span className="text-[14px] text-copper">⌘</span>
        <span className="hidden md:inline font-medium">Search or command…</span>
        <span className="kbd text-[11px] ml-1 hidden sm:inline border border-border-subtle bg-surface-3 px-1.5 py-0.5 rounded text-faint">Ctrl K</span>
      </button>

      <div className="flex-1 min-w-0" />

      {/* Provider & Model Selector */}
      <div className="relative shrink-0" ref={providerRef}>
        <button
          className={`flex h-8 items-center gap-2 rounded-lg border px-2.5 sm:px-3 text-[12px] sm:text-[13px] font-medium transition-all ${
            providerOpen
              ? "border-copper bg-copper/20 text-ink shadow-sm"
              : "border-border-subtle bg-surface-2 text-dim hover:border-copper/60 hover:text-ink"
          }`}
          onClick={() => setProviderOpen((o) => !o)}
          title="Change AI model (F6)"
          aria-expanded={providerOpen}
          aria-haspopup="menu"
        >
          <span className="h-2 w-2 rounded-full bg-copper shadow-[0_0_6px_rgba(255,46,77,0.8)]" />
          <span className="text-ink font-semibold">{provider || "AI"}</span>
          <span className="text-faint">/</span>
          <span className="max-w-[100px] sm:max-w-[150px] truncate text-copper font-semibold">{model || "Select"}</span>
          <span className="text-[11px] text-faint">▾</span>
        </button>

        {providerOpen && (
          <div
            role="menu"
            className="glass absolute right-0 top-10 z-50 w-76 sm:w-84 overflow-hidden rounded-xl border border-border-subtle bg-surface-2 p-3 shadow-2xl backdrop-blur-xl animate-fade-in"
          >
            <div className="flex items-center justify-between border-b border-border-subtle pb-2 px-2">
              <span className="text-[11.5px] font-semibold uppercase tracking-wider text-faint">
                Select Model
              </span>
              <button
                className="text-[12px] font-medium text-copper hover:underline"
                onClick={() => {
                  setProviderOpen(false);
                  setView("settings");
                }}
              >
                API Keys ⚙
              </button>
            </div>

            <div className="max-h-72 overflow-y-auto space-y-2 pt-2 pr-1">
              {providers.map((p) => {
                const isCurrent = p.name === provider;
                return (
                  <div
                    key={p.name}
                    className={`rounded-lg p-2.5 transition-all ${
                      isCurrent
                        ? "bg-surface-3 border border-copper/60 shadow-sm"
                        : "bg-surface border border-border-subtle hover:border-copper/40"
                    }`}
                  >
                    <div className="flex w-full items-center justify-between">
                      <button
                        className="flex items-center gap-2 text-[13px] font-semibold text-ink capitalize"
                        onClick={() => selectProvider(p.name)}
                      >
                        <span
                          className={`h-2 w-2 rounded-full ${
                            p.has_key ? "bg-success shadow-[0_0_6px_rgba(34,197,94,0.8)]" : "bg-faint"
                          }`}
                        />
                        {p.name}
                      </button>
                      <span className="text-[11px] text-faint">
                        {p.has_key ? "Configured" : "No Key"}
                      </span>
                    </div>

                    {isCurrent && models.length > 0 && (
                      <div className="mt-2 flex flex-wrap gap-1.5 border-t border-border-subtle pt-2">
                        {models.slice(0, 8).map((m) => (
                          <button
                            key={m}
                            className={`rounded-md px-2.5 py-1 text-[11.5px] font-medium transition-colors ${
                              model === m
                                ? "bg-copper text-white font-bold shadow-sm"
                                : "bg-surface-2 text-dim hover:text-ink hover:bg-surface-3 border border-border-subtle"
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

            <div className="border-t border-border-subtle mt-2.5 pt-2 px-1">
              <button
                className="w-full text-center text-[12px] text-dim hover:text-ink py-1.5 rounded bg-surface-3 transition-colors border border-border-subtle"
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
        className="flex h-8 items-center gap-1.5 rounded-lg border border-border-subtle bg-surface-2 px-2.5 text-[12px] font-medium text-dim hover:border-copper hover:text-ink transition-colors shrink-0"
        onClick={spawnTui}
        title="Open TUI terminal mode (F5)"
      >
        <span className="font-mono text-copper font-bold">▮</span>
        <span>TUI</span>
      </button>

      {/* Connection indicator */}
      <div
        className={`flex items-center gap-1.5 text-[12px] font-medium hidden sm:flex shrink-0 ${online ? "text-dim" : "text-warn"}`}
        title={online ? "Online" : "Disconnected"}
      >
        <span
          className={`h-2 w-2 rounded-full ${
            online ? "bg-success shadow-[0_0_6px_rgba(34,197,94,0.8)]" : "bg-warn"
          }`}
        />
        <span>{online ? "Online" : "Offline"}</span>
      </div>

      <div className="mx-0.5 h-4 w-px bg-border-subtle hidden sm:block" />

      {/* Window controls [–] [□] [×] */}
      <div className="flex items-center gap-0.5 rounded-lg border border-border-subtle bg-surface-2 p-0.5 shrink-0">
        <button
          className="flex h-7 w-7 items-center justify-center rounded text-[14px] font-medium text-dim hover:bg-surface-3 hover:text-ink transition-colors"
          onClick={() => windowAction("minimize")}
          title="Minimize"
          aria-label="Minimize"
        >
          –
        </button>
        <button
          className="flex h-7 w-7 items-center justify-center rounded text-[12px] font-medium text-dim hover:bg-surface-3 hover:text-ink transition-colors"
          onClick={() => windowAction("maximize")}
          title="Maximize"
          aria-label="Maximize"
        >
          □
        </button>
        <button
          className="flex h-7 w-7 items-center justify-center rounded text-[14px] font-medium text-dim hover:bg-copper/25 hover:text-copper transition-colors"
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
