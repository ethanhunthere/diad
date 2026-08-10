import { useEffect, useRef, useState } from "react";
import { Eye, EyeOff, KeyRound } from "lucide-react";
import { useStore, sendMsg } from "../stores/useStore";

/**
 * Masked input modal — shown when the daemon asks for a secret
 * (API key, Wi-Fi password). The value is only sent to the daemon,
 * never stored in the browser.
 */
export function MaskedInputModal() {
  const pending = useStore((s) => s.pendingSecret);
  const setPendingSecret = useStore((s) => s.setPendingSecret);
  const [value, setValue] = useState("");
  const [show, setShow] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (pending) {
      setValue("");
      setShow(false);
      setTimeout(() => inputRef.current?.focus(), 30);
    }
  }, [pending?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!pending) return null;

  const cancel = () => {
    sendMsg({ type: "secret_cancel", id: pending.id });
    setPendingSecret(null);
  };

  const submit = () => {
    if (!value) return;
    sendMsg({ type: "secret_submit", id: pending.id, value });
    setPendingSecret(null);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm animate-fade-in">
      <div className="glass w-[420px] rounded-panel p-5 shadow-2xl">
        <div className="flex items-center gap-2.5">
          <div className="flex h-9 w-9 items-center justify-center rounded-panel border border-copper/30 bg-copper/10 text-copper-hover">
            <KeyRound size={17} />
          </div>
          <div>
            <div className="text-sm font-semibold text-ink">Enter secret</div>
            <div className="text-[11.5px] text-dim">{pending.label}</div>
          </div>
        </div>

        <div className="relative mt-4">
          <input
            ref={inputRef}
            type={show ? "text" : "password"}
            value={value}
            autoFocus
            placeholder="Hidden input — never stored"
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") submit();
              if (e.key === "Escape") cancel();
            }}
            className="control w-full pr-9 font-mono"
          />
          <button
            className="absolute right-2 top-1/2 -translate-y-1/2 text-faint hover:text-ink"
            onClick={() => setShow((v) => !v)}
            title={show ? "Hide" : "Show"}
          >
            {show ? <EyeOff size={15} /> : <Eye size={15} />}
          </button>
        </div>

        <div className="mt-4 flex justify-end gap-2">
          <button className="btn-ghost" onClick={cancel}>
            Cancel (Esc)
          </button>
          <button className="btn-primary" disabled={!value} onClick={submit}>
            Submit (Enter)
          </button>
        </div>
      </div>
    </div>
  );
}
