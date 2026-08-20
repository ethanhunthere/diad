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
    <footer className="z-30 flex h-7 shrink-0 items-center justify-between border-t border-[#202623] bg-[#0F1110] px-4 text-[12px] text-[#8A968E] select-none font-sans">
      <div className="flex items-center gap-2">
        <span className={`h-2 w-2 rounded-full ${connected ? "bg-[#38A169]" : "bg-[#D97706]"}`} />
        <span className="font-semibold text-[#E5EAE6]">{provider || "DIAD"}</span>
        <span className="text-[#525D56]">/</span>
        <span className="text-[#8A968E]">{model || "Agent"}</span>
        {streaming && <span className="text-[#B32032] font-medium">· executing…</span>}
      </div>

      <div className="flex items-center gap-3 text-[#525D56] text-[11.5px] font-mono">
        <span>DIAD OS · Protocol v1</span>
      </div>
    </footer>
  );
}
