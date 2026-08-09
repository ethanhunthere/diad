import { FormEvent, useEffect, useRef, useState } from "react";
import { AlertTriangle, Eye, EyeOff, KeyRound, X } from "lucide-react";

import { useStore } from "../store";

/** Secret modal — masked input; value never logged, delivered only via WS. */
export function SecretModal() {
  const pending = useStore((s) => s.pendingSecret);
  const submit = useStore((s) => s.submitSecret);
  const cancel = useStore((s) => s.cancelSecret);

  const [value, setValue] = useState("");
  const [show, setShow] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (pending) {
      setValue("");
      setShow(false);
      window.setTimeout(() => inputRef.current?.focus(), 10);
    }
  }, [pending]);

  if (!pending) return null;

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    if (value.trim()) submit(value);
  };

  return (
    <div className="fixed inset-0 z-[70] flex items-center justify-center bg-black/60 backdrop-blur-[3px]">
      <div className="glass w-[min(420px,92vw)] animate-fade-in rounded-panel p-5 shadow-2xl">
        <div className="flex items-center gap-2.5">
          <KeyRound size={17} className="text-copper" />
          <h2 className="flex-1 text-[14px] font-semibold text-ink">Input required</h2>
          <button className="btn-icon h-6 w-6" onClick={cancel}>
            <X size={13} />
          </button>
        </div>
        <p className="mt-2 text-[13px] leading-relaxed text-dim">{pending.label}</p>
        <form onSubmit={onSubmit} className="mt-4">
          <div className="relative">
            <input
              ref={inputRef}
              type={show ? "text" : "password"}
              value={value}
              onChange={(e) => setValue(e.target.value)}
              className="input pr-10 font-mono"
              autoComplete="off"
              spellCheck={false}
              placeholder="••••••••••••"
            />
            <button
              type="button"
              onClick={() => setShow((v) => !v)}
              className="absolute right-2.5 top-1/2 -translate-y-1/2 text-faint transition-colors hover:text-ink"
            >
              {show ? <EyeOff size={14} /> : <Eye size={14} />}
            </button>
          </div>
          <div className="mt-4 flex items-center justify-between gap-2">
            <span className="flex items-center gap-1.5 text-[10.5px] text-faint">
              <AlertTriangle size={10} />
              sent only to the DIAD daemon — never stored in chat
            </span>
            <div className="flex gap-2">
              <button type="button" onClick={cancel} className="btn-ghost">
                Cancel
              </button>
              <button type="submit" disabled={!value.trim()} className="btn-copper">
                Submit
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
}

/** Confirm modal — explicit yes/no for destructive tool calls. */
export function ConfirmModal() {
  const pending = useStore((s) => s.pendingConfirm);
  const answer = useStore((s) => s.answerConfirm);

  if (!pending) return null;

  return (
    <div className="fixed inset-0 z-[70] flex items-center justify-center bg-black/60 backdrop-blur-[3px]">
      <div className="glass w-[min(420px,92vw)] animate-fade-in rounded-panel p-5 shadow-2xl">
        <div className="flex items-center gap-2.5">
          <AlertTriangle size={17} className="text-warn" />
          <h2 className="flex-1 text-[14px] font-semibold text-ink">Confirm action</h2>
        </div>
        <p className="mt-2 text-[13px] leading-relaxed text-dim">{pending.label}</p>
        <div className="mt-4 flex justify-end gap-2">
          <button onClick={() => answer(false)} className="btn-ghost">
            Reject
          </button>
          <button onClick={() => answer(true)} className="btn-copper">
            Approve
          </button>
        </div>
      </div>
    </div>
  );
}
