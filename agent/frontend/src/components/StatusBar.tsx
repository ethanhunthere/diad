import { Battery, Signal, Wifi, WifiOff } from "lucide-react";

import { useStore } from "../store";

/** Bottom status bar: mode · provider·model · tokens · latency · battery/wifi · cwd. */
export default function StatusBar() {
  const provider = useStore((s) => s.provider);
  const model = useStore((s) => s.model);
  const connected = useStore((s) => s.connected);
  const ws = useStore((s) => s.ws);
  const stats = useStore((s) => s.stats);

  const wifiOn = stats.wifi !== "offline" && stats.wifi !== "n/a";
  const battery = stats.battery !== "n/a" ? stats.battery : null;

  return (
    <footer className="glass z-30 flex h-6 shrink-0 items-center gap-4 border-t border-border-subtle px-3 text-[11px] text-dim">
      <span className="flex items-center gap-1.5 font-semibold text-ink">
        <span
          className={`h-1.5 w-1.5 rounded-full ${
            ws === "connected" ? (connected ? "bg-success" : "bg-warn") : "bg-danger"
          }`}
        />
        {ws === "connected" ? (connected ? "CONNECTED" : "NO KEY") : ws.toUpperCase()}
      </span>

      <span className="text-faint">
        {provider}
        {model ? ` · ${model}` : ""}
      </span>

      <span className="hidden sm:inline">tokens {stats.tokens.toLocaleString()}</span>
      <span className="hidden sm:inline">
        {stats.latency > 0 ? `${stats.latency} ms` : "— ms"}
      </span>

      <span className="flex-1" />

      {battery && (
        <span className="flex items-center gap-1" title={`Battery ${battery}`}>
          <Battery size={12} />
          {battery}
        </span>
      )}
      {wifiOn ? (
        <span className="flex items-center gap-1" title={`Wi-Fi ${stats.wifi}`}>
          <Wifi size={12} />
        </span>
      ) : (
        <span className="flex items-center gap-1 text-faint" title="Wi-Fi offline">
          <WifiOff size={12} />
        </span>
      )}
      {stats.battery !== "n/a" && battery === null && (
        <Signal size={12} className="text-faint" />
      )}

      <span className="max-w-[280px] truncate font-mono text-[10.5px] text-faint" title={stats.cwd}>
        {stats.cwd}
      </span>
    </footer>
  );
}
