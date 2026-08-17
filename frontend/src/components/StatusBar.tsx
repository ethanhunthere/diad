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
    <footer className="z-30 flex h-6 shrink-0 items-center justify-between border-t border-border-subtle/80 bg-[#12141d] px-4 text-[11px] text-slate-400 select-none">
      <div className="flex items-center gap-2 font-mono">
        <span className={`h-1.5 w-1.5 rounded-full ${connected ? "bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.8)]" : "bg-amber-400 shadow-[0_0_6px_rgba(245,158,11,0.8)]"}`} />
        <span className="font-bold text-slate-200">{provider || "DIAD"}</span>
        <span className="text-slate-600">/</span>
        <span className="text-slate-300">{model || "Agent"}</span>
        {streaming && <span className="text-amber-400 animate-pulse font-normal">· generating…</span>}
      </div>

      <div className="flex items-center gap-3 text-slate-500 text-[11px] font-mono">
        <span>DIAD Autonomous OS</span>
      </div>
    </footer>
  );
}
