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
    <header className="glass z-30 flex h-12 shrink-0 items-center gap-2 sm:gap-3 border-b border-[#202623] px-3 sm:px-4 select-none bg-[#0F1110]">
      {hamburger}

      {/* DIAD Wordmark */}
      <DiadLogo size={16} />

      <div className="mx-1 h-4 w-px bg-[#202623] hidden sm:block" />

      {/* Quick search / command trigger */}
      <button
        className="flex h-8 items-center gap-2 rounded-lg border border-[#202623] bg-[#161918] px-2.5 sm:px-3 text-[12px] sm:text-[13px] text-[#8A968E] hover:border-[#B32032] hover:text-[#E5EAE6] transition-colors"
        onClick={() => setPaletteOpen(true)}
        title="Open command palette (Ctrl+K)"
      >
        <span className="text-[14px]">⌘</span>
        <span className="hidden md:inline">Search or command…</span>
        <span className="kbd text-[11px] ml-1 hidden sm:inline">Ctrl K</span>
      </button>

      <div className="flex-1 min-w-0" />

      {/* Provider & Model Selector */}
      <div className="relative shrink-0" ref={providerRef}>
        <button
          className={`flex h-8 items-center gap-2 rounded-lg border px-2.5 sm:px-3 text-[12px] sm:text-[13px] font-medium transition-all ${
            providerOpen
              ? "border-[#B32032] bg-[#B32032]/20 text-[#E5EAE6] shadow-sm"
              : "border-[#202623] bg-[#161918] text-[#8A968E] hover:border-[#B32032]/60 hover:text-[#E5EAE6]"
          }`}
          onClick={() => setProviderOpen((o) => !o)}
          title="Change AI model (F6)"
          aria-expanded={providerOpen}
          aria-haspopup="menu"
        >
          <span className="h-2 w-2 rounded-full bg-[#B32032]" />
          <span className="text-[#E5EAE6] font-semibold">{provider || "AI"}</span>
          <span className="text-[#525D56]">/</span>
          <span className="max-w-[100px] sm:max-w-[150px] truncate text-[#B32032] font-semibold">{model || "Select"}</span>
          <span className="text-[11px] text-[#525D56]">▾</span>
        </button>

        {providerOpen && (
          <div
            role="menu"
            className="glass absolute right-0 top-10 z-50 w-76 sm:w-84 overflow-hidden rounded-xl border border-[#202623] bg-[#161918] p-3 shadow-2xl backdrop-blur-xl animate-fade-in"
          >
            <div className="flex items-center justify-between border-b border-[#202623] pb-2 px-2">
              <span className="text-[11.5px] font-semibold uppercase tracking-wider text-[#8A968E]">
                Select Model
              </span>
              <button
                className="text-[12px] font-medium text-[#B32032] hover:underline"
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
                        ? "bg-[#1E2321] border border-[#B32032]/60 shadow-sm"
                        : "bg-[#0F1110] border border-[#202623] hover:border-[#B32032]/30"
                    }`}
                  >
                    <div className="flex w-full items-center justify-between">
                      <button
                        className="flex items-center gap-2 text-[13px] font-semibold text-[#E5EAE6] capitalize"
                        onClick={() => selectProvider(p.name)}
                      >
                        <span
                          className={`h-2 w-2 rounded-full ${
                            p.has_key ? "bg-[#38A169]" : "bg-[#525D56]"
                          }`}
                        />
                        {p.name}
                      </button>
                      <span className="text-[11px] text-[#8A968E]">
                        {p.has_key ? "Configured" : "No Key"}
                      </span>
                    </div>

                    {isCurrent && models.length > 0 && (
                      <div className="mt-2 flex flex-wrap gap-1.5 border-t border-[#202623] pt-2">
                        {models.slice(0, 8).map((m) => (
                          <button
                            key={m}
                            className={`rounded-md px-2.5 py-1 text-[11.5px] font-medium transition-colors ${
                              model === m
                                ? "bg-[#B32032] text-white font-bold shadow-sm"
                                : "bg-[#161918] text-[#8A968E] hover:text-[#E5EAE6] hover:bg-[#1E2321]"
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

            <div className="border-t border-[#202623] mt-2.5 pt-2 px-1">
              <button
                className="w-full text-center text-[12px] text-[#8A968E] hover:text-white py-1.5 rounded bg-[#1E2321] transition-colors"
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
        className="flex h-8 items-center gap-1.5 rounded-lg border border-[#202623] bg-[#161918] px-2.5 text-[12px] font-medium text-[#8A968E] hover:border-[#B32032] hover:text-[#E5EAE6] transition-colors shrink-0"
        onClick={spawnTui}
        title="Open TUI terminal mode (F5)"
      >
        <span className="font-mono text-[#B32032] font-bold">▮</span>
        <span>TUI</span>
      </button>

      {/* Connection indicator (Clean, solid without pulsing shadows) */}
      <div
        className={`flex items-center gap-1.5 text-[12px] font-medium hidden sm:flex shrink-0 ${online ? "text-[#8A968E]" : "text-[#D97706]"}`}
        title={online ? "Online" : "Disconnected"}
      >
        <span
          className={`h-2 w-2 rounded-full ${
            online ? "bg-[#38A169]" : "bg-[#D97706]"
          }`}
        />
        <span>{online ? "Online" : "Offline"}</span>
      </div>

      <div className="mx-0.5 h-4 w-px bg-[#202623] hidden sm:block" />

      {/* Window controls [–] [□] [×] */}
      <div className="flex items-center gap-0.5 rounded-lg border border-[#202623] bg-[#161918] p-0.5 shrink-0">
        <button
          className="flex h-7 w-7 items-center justify-center rounded text-[14px] font-medium text-[#8A968E] hover:bg-[#1E2321] hover:text-[#E5EAE6] transition-colors"
          onClick={() => windowAction("minimize")}
          title="Minimize"
          aria-label="Minimize"
        >
          –
        </button>
        <button
          className="flex h-7 w-7 items-center justify-center rounded text-[12px] font-medium text-[#8A968E] hover:bg-[#1E2321] hover:text-[#E5EAE6] transition-colors"
          onClick={() => windowAction("maximize")}
          title="Maximize"
          aria-label="Maximize"
        >
          □
        </button>
        <button
          className="flex h-7 w-7 items-center justify-center rounded text-[14px] font-medium text-[#8A968E] hover:bg-[#B32032]/25 hover:text-[#B32032] transition-colors"
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
