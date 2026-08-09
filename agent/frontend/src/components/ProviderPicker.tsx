import { useEffect, useMemo, useState } from "react";
import { Check, ChevronRight, KeyRound, X } from "lucide-react";

import { useStore } from "../store";

/** F6 provider/model picker. */
export default function ProviderPicker() {
  const open = useStore((s) => s.providerPickerOpen);
  const setOpen = useStore((s) => s.setProviderPickerOpen);
  const providers = useStore((s) => s.providers);
  const provider = useStore((s) => s.provider);
  const model = useStore((s) => s.model);
  const models = useStore((s) => s.models);
  const switchProvider = useStore((s) => s.switchProvider);
  const switchModel = useStore((s) => s.switchModel);
  const requestModels = useStore((s) => s.requestModels);
  const requestProviders = useStore((s) => s.requestProviders);

  const [showModelsFor, setShowModelsFor] = useState<string | null>(provider);
  const [q, setQ] = useState("");

  useEffect(() => {
    if (open) {
      setQ("");
      setShowModelsFor(provider);
      requestProviders();
      requestModels(provider);
    }
  }, [open, provider, requestModels, requestProviders]);

  const filtered = useMemo(
    () =>
      providers.filter((p) =>
        q.trim() ? p.name.toLowerCase().includes(q.toLowerCase()) : true
      ),
    [providers, q]
  );

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/55 pt-[16vh] backdrop-blur-[2px]"
      onMouseDown={() => setOpen(false)}
    >
      <div
        className="glass w-[min(480px,92vw)] overflow-hidden rounded-panel shadow-2xl"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-border-subtle px-3 py-2.5">
          <span className="text-[12px] font-semibold uppercase tracking-widest text-copper">
            Provider
          </span>
          <input
            autoFocus
            value={q}
            onChange={(e) => setQ(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") setOpen(false);
            }}
            placeholder="Filter providers…"
            className="flex-1 bg-transparent px-1 text-[13.5px] text-ink placeholder:text-faint outline-none"
          />
          <button className="btn-icon h-6 w-6" onClick={() => setOpen(false)}>
            <X size={13} />
          </button>
        </div>

        <div className="max-h-[320px] overflow-y-auto py-1.5">
          {filtered.map((p) => {
            const active = p.name === provider;
            const expanded = showModelsFor === p.name;
            return (
              <div key={p.name}>
                <button
                  onClick={() => {
                    switchProvider(p.name);
                    setShowModelsFor(p.name);
                    requestModels(p.name);
                  }}
                  className={`flex w-full items-center gap-2.5 px-3 py-2 text-left transition-colors ${
                    active ? "bg-copper/15" : "hover:bg-surface-2/70"
                  }`}
                >
                  <span
                    className={`flex h-5 w-5 items-center justify-center rounded-full border ${
                      active ? "border-copper bg-copper/30 text-copper" : "border-border-strong text-transparent"
                    }`}
                  >
                    <Check size={11} />
                  </span>
                  <span className="w-32 truncate font-mono text-[13px] font-semibold text-ink">
                    {p.name}
                  </span>
                  <span className="min-w-0 flex-1 truncate text-[11px] text-faint">
                    {p.has_key
                      ? "key set"
                      : p.needs_key
                        ? "needs API key"
                        : "local endpoint"}
                  </span>
                  {!p.needs_key && !p.has_key && <KeyRound size={12} className="text-faint" />}
                  <ChevronRight
                    size={13}
                    className={`text-faint transition-transform duration-150 ${expanded ? "rotate-90" : ""}`}
                  />
                </button>

                {expanded && (
                  <div className="border-l border-border-subtle pb-1 pl-8 pr-2">
                    {models.map((m) => (
                      <button
                        key={m}
                        onClick={() => {
                          switchModel(m);
                          setOpen(false);
                        }}
                        className={`flex w-full items-center gap-2 rounded-control px-2 py-1 text-left font-mono text-[12px] transition-colors ${
                          m === model
                            ? "bg-copper/10 text-copper"
                            : "text-dim hover:bg-surface-2 hover:text-ink"
                        }`}
                      >
                        {m === model && <Check size={11} />}
                        <span className="min-w-0 flex-1 truncate">{m}</span>
                      </button>
                    ))}
                    {!models.length && (
                      <div className="px-2 py-1.5 text-[11px] text-faint">
                        No models listed yet…
                      </div>
                    )}
                  </div>
                )}
              </div>
            );
          })}
          {!filtered.length && (
            <div className="px-3 py-6 text-center text-[12px] text-faint">
              No providers match “{q}”. Run <code className="font-mono text-copper">/connect</code> in chat.
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
