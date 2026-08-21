import { useStore } from "../stores/useStore";

/**
 * Clean minimal status footer — connection status, active provider & model.
 */
export function StatusBar() {
  const provider = useStore((s) => s.provider);
  const model = useStore((s) => s.model);
  const connected = useStore((s) => s.connected);
  const streaming = useStore((s) => s.streaming);

  return (
    <footer className="z-30 flex h-7 shrink-0 items-center justify-between border-t border-border-subtle bg-surface px-4 text-[12px] text-dim select-none font-sans">
      <div className="flex items-center gap-2">
        <span className={`h-2 w-2 rounded-full ${connected ? "bg-success shadow-[0_0_6px_rgba(34,197,94,0.8)]" : "bg-warn"}`} />
        <span className="font-semibold text-ink">{provider || "DIAD"}</span>
        <span className="text-faint">/</span>
        <span className="text-dim font-medium">{model || "Agent"}</span>
        {streaming && <span className="text-copper font-medium">· executing…</span>}
      </div>

      <div className="flex items-center gap-3 text-faint text-[11.5px] font-mono">
        <span>DIAD OS · Protocol v1</span>
      </div>
    </footer>
  );
}
