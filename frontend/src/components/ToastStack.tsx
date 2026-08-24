import { CheckCircle2, Info, AlertTriangle, XCircle, X } from "lucide-react";
import { useStore } from "../stores/useStore";

const ICONS = {
  info: Info,
  success: CheckCircle2,
  warn: AlertTriangle,
  error: XCircle,
};

const COLORS = {
  info: "text-dim border-border-subtle",
  success: "text-success border-success/30",
  warn: "text-warn border-warn/30",
  error: "text-danger border-danger/30",
};

/**
 * Toast stack — bottom-right, auto-dismissing notifications.
 */
export function ToastStack() {
  const toasts = useStore((s) => s.toasts);
  const dismissToast = useStore((s) => s.dismissToast);

  if (toasts.length === 0) return null;

  return (
    <div className="pointer-events-none fixed bottom-8 right-3 z-40 flex w-80 flex-col gap-2">
      {toasts.map((t) => {
        const Icon = ICONS[t.kind];
        return (
          <div
            key={t.id}
            className={`pointer-events-auto glass flex items-start gap-2.5 rounded-panel border p-3 shadow-xl animate-fade-in ${COLORS[t.kind]}`}
          >
            <Icon size={15} className="mt-0.5 shrink-0" />
            <div className="min-w-0 flex-1">
              <div className="text-[12px] font-medium text-ink">{t.title}</div>
              {t.message && (
                <div className="mt-0.5 break-words text-[11px] text-dim">{t.message}</div>
              )}
            </div>
            <button
              className="shrink-0 text-faint hover:text-ink"
              onClick={() => dismissToast(t.id)}
              aria-label="Dismiss notification"
            >
              <X size={13} />
            </button>
          </div>
        );
      })}
    </div>
  );
}
