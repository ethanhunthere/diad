import { useEffect, useRef } from "react";
import { AlertTriangle, Check, X } from "lucide-react";
import { useStore, sendMsg } from "../stores/useStore";
import { useFocusTrap } from "./useFocusTrap";

/**
 * Confirmation modal — shown when the daemon needs an explicit
 * yes/no (destructive tools). Maps to tool_approve / tool_reject.
 */
export function ConfirmModal() {
  const pending = useStore((s) => s.pendingConfirm);
  const setPendingConfirm = useStore((s) => s.setPendingConfirm);
  const dialogRef = useRef<HTMLDivElement>(null);

  const answer = (yes: boolean) => {
    if (!pending) return;
    sendMsg({ type: yes ? "tool_approve" : "tool_reject", id: pending.id });
    setPendingConfirm(null);
  };

  // F-16: focus trap + Escape parity + focus restore.
  useFocusTrap(dialogRef, { active: !!pending, onEscape: () => answer(false) });

  useEffect(() => {
    if (!pending) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "y" || e.key === "Y") answer(true);
      if (e.key === "n" || e.key === "N") answer(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [pending?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!pending) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm animate-fade-in">
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-label="Confirmation required"
        className="glass w-[420px] rounded-panel p-5 shadow-2xl"
      >
        <div className="flex items-center gap-2.5">
          <div className="flex h-9 w-9 items-center justify-center rounded-panel border border-warn/30 bg-warn/10 text-warn">
            <AlertTriangle size={17} />
          </div>
          <div>
            <div className="text-sm font-semibold text-ink">Confirmation required</div>
            <div className="mt-0.5 text-[12px] text-dim">{pending.label}</div>
          </div>
        </div>

        <div className="mt-5 flex justify-end gap-2">
          <button className="btn-ghost" onClick={() => answer(false)}>
            <X size={14} /> Reject (N)
          </button>
          <button className="btn-primary" onClick={() => answer(true)} autoFocus>
            <Check size={14} /> Approve (Y)
          </button>
        </div>
        <div className="mt-3 text-center text-[10px] text-faint">Press Y or N</div>
      </div>
    </div>
  );
}
