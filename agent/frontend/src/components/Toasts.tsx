import { AlertTriangle, CheckCircle2, Info, X, XCircle } from "lucide-react";

import { useStore } from "../store";

const ICONS = {
  info: <Info size={14} className="text-copper" />,
  success: <CheckCircle2 size={14} className="text-success" />,
  warn: <AlertTriangle size={14} className="text-warn" />,
  error: <XCircle size={14} className="text-danger" />,
};

/** Stacked toasts, top-right. */
export default function Toasts() {
  const toasts = useStore((s) => s.toasts);
  const dismiss = useStore((s) => s.dismissToast);

  return (
    <div className="pointer-events-none fixed right-4 top-12 z-[60] flex w-80 flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          className="glass pointer-events-auto flex animate-fade-in items-start gap-2.5 rounded-panel px-3.5 py-2.5 shadow-xl"
        >
          <span className="mt-0.5 shrink-0">{ICONS[t.kind]}</span>
          <span className="min-w-0 flex-1 text-[12.5px] leading-snug text-ink">{t.text}</span>
          <button
            onClick={() => dismiss(t.id)}
            className="shrink-0 rounded p-0.5 text-faint transition-colors hover:text-ink"
          >
            <X size={12} />
          </button>
        </div>
      ))}
    </div>
  );
}
