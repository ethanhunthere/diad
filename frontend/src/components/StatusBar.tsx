import { useEffect } from "react";
import { BatteryMedium, Wifi, HardDrive } from "lucide-react";
import { useStore, sendMsg } from "../store/useStore";
import { nextId, shortenPath } from "../lib/format";

/**
 * Bottom status bar — mode, provider·model, tokens/session,
 * latency, battery, wifi, workspace.
 */
export function StatusBar() {
  const provider = useStore((s) => s.provider);
  const model = useStore((s) => s.model);
  const connected = useStore((s) => s.connected);
  const streaming = useStore((s) => s.streaming);
  const stats = useStore((s) => s.stats);
  const workspace = useStore((s) => s.workspace);
  const view = useStore((s) => s.view);
  const wsConnected = useStore((s) => s.wsConnected);

  // Poll stats every 5s while connected.
  useEffect(() => {
    if (!wsConnected) return;
    const t = setInterval(() => {
      sendMsg({ type: "stats.get", id: nextId("st") });
    }, 5000);
    return () => clearInterval(t);
  }, [wsConnected]);

  const modeLabel =
    view === "chat"
      ? streaming
        ? "STREAMING"
        : "CHAT"
      : view.toUpperCase();

  return (
    <footer className="glass z-30 flex h-6 shrink-0 items-center gap-3 border-t border-border-subtle px-3 text-[10.5px] text-dim select-none">
      <span className={streaming ? "text-copper-hover" : ""}>{modeLabel}</span>
      <span className="text-faint">|</span>

      <span title="Provider · model">
        {provider || "no provider"} · {model || "no model"}
      </span>
      <span className="text-faint">|</span>

      <span title="Tokens this session">{stats.tokens} tok</span>
      <span className="text-faint">|</span>

      <span
        title="Last latency"
        className={stats.latency_ms > 0 ? (stats.latency_ms > 4000 ? "text-warn" : "text-success") : ""}
      >
        {stats.latency_ms > 0 ? `${stats.latency_ms} ms` : "— ms"}
      </span>
      <span className="text-faint">|</span>

      <span className="flex items-center gap-1" title="Battery">
        <BatteryMedium size={11} />
        {stats.battery || "n/a"}
      </span>
      <span className="text-faint">|</span>

      <span className="flex items-center gap-1" title="Wi-Fi">
        <Wifi size={11} />
        {stats.wifi || "offline"}
      </span>

      <div className="flex-1" />

      <span className="flex items-center gap-1 truncate" title={workspace}>
        <HardDrive size={11} />
        {shortenPath(workspace || "~", 40)}
      </span>

      <span className={`h-1.5 w-1.5 rounded-full ${connected ? "bg-success" : "bg-warn"}`} />
    </footer>
  );
}
