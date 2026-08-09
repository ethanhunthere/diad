import { FormEvent, useEffect, useMemo, useState } from "react";
import { ArrowLeft, ArrowRight, Check, KeyRound, Sparkles } from "lucide-react";

import { useStore } from "../store";

const STORAGE_KEY = "diad-agent.onboarded.v1";

/** First-launch wizard: welcome → connect provider → done. */
export default function Onboarding() {
  const open = useStore((s) => s.onboarding);
  const setOnboarding = useStore((s) => s.setOnboarding);
  const providers = useStore((s) => s.providers);
  const connected = useStore((s) => s.connected);
  const connectProvider = useStore((s) => s.connectProvider);

  const [step, setStep] = useState(0);
  const [chosen, setChosen] = useState<string>("");
  const [key, setKey] = useState<string>("");

  const connectable = useMemo(
    () => providers.filter((p) => p.needs_key && !p.has_key),
    [providers]
  );

  useEffect(() => {
    if (localStorage.getItem(STORAGE_KEY)) return;
    if (open && providers.length) {
      setStep(0);
      setChosen(connectable[0]?.name ?? "");
    }
  }, [open, providers, connectable]);

  const finish = () => {
    localStorage.setItem(STORAGE_KEY, "1");
    setOnboarding(false);
  };

  const onConnect = (e: FormEvent) => {
    e.preventDefault();
    if (chosen && key.trim()) {
      connectProvider(chosen, key.trim());
      window.setTimeout(() => setStep(2), 900);
    }
  };

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-[80] flex items-center justify-center bg-black/70 backdrop-blur-[4px]">
      <div className="glass w-[min(520px,92vw)] animate-fade-in overflow-hidden rounded-panel shadow-2xl">
        {/* header */}
        <div className="flex items-center gap-3 border-b border-border-subtle px-6 py-5">
          <span className="flex h-10 w-10 items-center justify-center rounded-panel bg-copper font-mono text-lg font-bold text-[#1a0f07]">
            D
          </span>
          <div>
            <h1 className="text-[15px] font-bold tracking-[0.14em] text-ink">
              DIAD<span className="text-copper"> AGENT</span>
            </h1>
            <p className="text-[11.5px] text-dim">Welcome to your intent-driven OS</p>
          </div>
          <div className="flex-1" />
          {/* step dots */}
          <div className="flex gap-1.5">
            {[0, 1, 2].map((i) => (
              <span
                key={i}
                className={`h-1.5 rounded-full transition-all duration-200 ${
                  i === step ? "w-6 bg-copper" : i < step ? "w-1.5 bg-copper-dim" : "w-1.5 bg-border-strong"
                }`}
              />
            ))}
          </div>
        </div>

        <div className="px-6 py-6">
          {step === 0 && (
            <div>
              <Sparkles size={20} className="text-copper" />
              <h2 className="mt-2 text-[15px] font-semibold text-ink">
                One daemon, two faces
              </h2>
              <p className="mt-2 text-[13px] leading-relaxed text-dim">
                DIAD AGENT runs as a background daemon on this machine. This GUI is its primary
                face; the terminal TUI (Ctrl+Shift+X) stays in sync. Everything — chat, files,
                editor, terminal, agents — speaks one protocol.
              </p>
              <ul className="mt-4 space-y-2 text-[12.5px] text-ink/90">
                <li className="flex items-center gap-2">
                  <Check size={13} className="text-success" /> Chat with local + cloud LLM providers
                </li>
                <li className="flex items-center gap-2">
                  <Check size={13} className="text-success" /> Managed file editor &amp; live PTY
                </li>
                <li className="flex items-center gap-2">
                  <Check size={13} className="text-success" /> Delegate to CLI coding agents
                </li>
              </ul>
            </div>
          )}

          {step === 1 && (
            <form onSubmit={onConnect}>
              <h2 className="text-[15px] font-semibold text-ink">Connect a provider</h2>
              <p className="mt-1 text-[12.5px] text-dim">
                Pick a provider and paste its API key. Keys are stored in the system keyring and
                never leave this machine.
              </p>
              <div className="mt-4 grid grid-cols-2 gap-2">
                {connectable.slice(0, 8).map((p) => (
                  <button
                    key={p.name}
                    type="button"
                    onClick={() => setChosen(p.name)}
                    className={`rounded-control border px-3 py-2 text-left font-mono text-[12.5px] transition-colors ${
                      chosen === p.name
                        ? "border-copper bg-copper/15 text-ink"
                        : "border-border-subtle bg-surface text-dim hover:bg-surface-2"
                    }`}
                  >
                    {p.name}
                  </button>
                ))}
              </div>
              <input
                type="password"
                value={key}
                onChange={(e) => setKey(e.target.value)}
                placeholder={`API key for ${chosen || "provider"}…`}
                autoComplete="off"
                spellCheck={false}
                className="input mt-3 font-mono"
              />
              <div className="mt-2 flex items-center gap-1.5 text-[11px] text-faint">
                <KeyRound size={11} />
                You can also skip this and use <code className="font-mono text-copper">/connect</code> later.
              </div>
            </form>
          )}

          {step === 2 && (
            <div>
              <Check size={20} className="text-success" />
              <h2 className="mt-2 text-[15px] font-semibold text-ink">
                {connected ? "You're connected!" : "All set"}
              </h2>
              <p className="mt-2 text-[13px] leading-relaxed text-dim">
                {connected
                  ? "The daemon is live and the provider key is stored. Ask your first question — try “what can you do?”."
                  : "You can connect a provider any time with F6 or /connect. Everything else works offline."}
              </p>
              <div className="mt-4 rounded-control bg-surface-2/60 px-3 py-2 font-mono text-[11.5px] text-dim">
                diad-agent · ws://127.0.0.1:7860/ws · protocol v1
              </div>
            </div>
          )}
        </div>

        {/* footer nav */}
        <div className="flex items-center justify-between border-t border-border-subtle px-6 py-4">
          {step > 0 ? (
            <button className="btn-ghost" onClick={() => setStep((s) => s - 1)}>
              <ArrowLeft size={13} />
              Back
            </button>
          ) : (
            <span />
          )}
          {step === 0 && (
            <button className="btn-copper" onClick={() => setStep(1)}>
              Continue
              <ArrowRight size={13} />
            </button>
          )}
          {step === 1 && (
            <div className="flex gap-2">
              <button className="btn-ghost" onClick={finish}>
                Skip
              </button>
              <button className="btn-copper" type="submit" onClick={onConnect} disabled={!chosen || !key.trim()}>
                Connect
              </button>
            </div>
          )}
          {step === 2 && (
            <button className="btn-copper" onClick={finish}>
              Start using DIAD AGENT
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
