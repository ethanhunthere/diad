import { FormEvent, useEffect, useMemo, useState } from "react";
import {
  Blocks,
  Bot,
  Check,
  ChevronDown,
  Cpu,
  KeyRound,
  Loader2,
  SendHorizonal,
} from "lucide-react";

import { useStore } from "../store";

const AGENT_META: Record<string, { blurb: string; accent: string }> = {
  claude: { blurb: "Anthropic CLI coding agent", accent: "#D97757" },
  codex: { blurb: "OpenAI CLI coding agent", accent: "#8FA8C8" },
  aider: { blurb: "AI pair programming in your terminal", accent: "#6FBF8B" },
  opencode: { blurb: "Open-source terminal coding agent", accent: "#C87F4A" },
  gemini: { blurb: "Google AI coding assistant", accent: "#7FC4C0" },
};

/** Agents view: detected external CLI agents + provider connection matrix. */
export default function AgentsView() {
  const agents = useStore((s) => s.agents);
  const providers = useStore((s) => s.providers);
  const requestAgents = useStore((s) => s.requestAgents);
  const requestProviders = useStore((s) => s.requestProviders);
  const delegate = useStore((s) => s.delegate);
  const connectProvider = useStore((s) => s.connectProvider);
  const toast = useStore((s) => s.toast);

  const [task, setTask] = useState<Record<string, string>>({});
  const [keys, setKeys] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<Record<string, boolean>>({});
  const [showKeys, setShowKeys] = useState<Record<string, boolean>>({});
  const [openProvider, setOpenProvider] = useState<string | null>(null);

  useEffect(() => {
    requestAgents();
    requestProviders();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const detected = useMemo(
    () => agents.filter((a) => a in AGENT_META),
    [agents]
  );
  const unknown = useMemo(
    () => agents.filter((a) => !(a in AGENT_META)),
    [agents]
  );

  const onDelegate = (e: FormEvent, agent: string) => {
    e.preventDefault();
    const t = (task[agent] ?? "").trim();
    if (!t) return;
    delegate(agent, t);
    setTask((s) => ({ ...s, [agent]: "" }));
    toast("info", `Delegated to ${agent} — output streams into chat`);
  };

  const onConnect = (e: FormEvent, name: string, needsKey: boolean) => {
    e.preventDefault();
    if (needsKey) {
      const key = (keys[name] ?? "").trim();
      if (!key) {
        toast("warn", `Paste an API key for ${name} first`);
        return;
      }
      setBusy((s) => ({ ...s, [name]: true }));
      connectProvider(name, key);
      window.setTimeout(
        () => setBusy((s) => ({ ...s, [name]: false })),
        1200
      );
    } else {
      setBusy((s) => ({ ...s, [name]: true }));
      connectProvider(name, "");
      window.setTimeout(
        () => setBusy((s) => ({ ...s, [name]: false })),
        1200
      );
    }
  };

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <div className="mx-auto max-w-4xl space-y-8 px-6 py-6">
        {/* External agents */}
        <section>
          <div className="mb-3 flex items-center gap-2">
            <Blocks size={16} className="text-copper" />
            <h2 className="text-[15px] font-semibold text-ink">External coding agents</h2>
            <span className="rounded bg-surface-2 px-1.5 py-0.5 text-[10.5px] text-dim">
              detected on PATH
            </span>
          </div>

          <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
            {detected.map((a) => {
              const meta = AGENT_META[a];
              return (
                <div key={a} className="panel overflow-hidden">
                  <div className="flex items-center gap-3 border-b border-border-subtle px-4 py-3">
                    <span
                      className="flex h-9 w-9 items-center justify-center rounded-control"
                      style={{ background: `${meta.accent}22`, color: meta.accent }}
                    >
                      <Bot size={18} />
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="font-mono text-[13.5px] font-bold text-ink">{a}</span>
                        <span className="rounded bg-success/15 px-1.5 py-0.5 text-[9.5px] font-bold uppercase text-success">
                          <Check size={9} className="mr-0.5 inline" />
                          ready
                        </span>
                      </div>
                      <p className="truncate text-[11.5px] text-dim">{meta.blurb}</p>
                    </div>
                    <Cpu size={14} className="text-faint" />
                  </div>
                  <form onSubmit={(e) => onDelegate(e, a)} className="flex gap-2 px-4 py-3">
                    <input
                      value={task[a] ?? ""}
                      onChange={(e) => setTask((s) => ({ ...s, [a]: e.target.value }))}
                      placeholder={`Delegate a task to ${a}…`}
                      className="input py-1.5 text-[12.5px]"
                    />
                    <button type="submit" className="btn-copper shrink-0 px-3 py-1.5">
                      <SendHorizonal size={13} />
                      Run
                    </button>
                  </form>
                </div>
              );
            })}
          </div>

          {unknown.length > 0 && (
            <div className="mt-2 flex flex-wrap gap-2">
              {unknown.map((a) => (
                <span
                  key={a}
                  className="rounded-control border border-border-subtle bg-surface-2 px-2 py-1 font-mono text-[11.5px] text-dim"
                >
                  {a}
                </span>
              ))}
            </div>
          )}

          {!detected.length && !unknown.length && (
            <div className="rounded-panel border border-dashed border-border-strong px-4 py-6 text-center text-[12.5px] text-faint">
              No external agents detected. Install one on PATH (e.g. <code className="font-mono text-copper">claude</code>,{" "}
              <code className="font-mono text-copper">codex</code>, <code className="font-mono text-copper">aider</code>)
              and restart DIAD AGENT.
            </div>
          )}
        </section>

        {/* Provider matrix */}
        <section>
          <div className="mb-3 flex items-center gap-2">
            <KeyRound size={16} className="text-copper" />
            <h2 className="text-[15px] font-semibold text-ink">Providers</h2>
            <span className="rounded bg-surface-2 px-1.5 py-0.5 text-[10.5px] text-dim">
              API keys stored in the system keyring
            </span>
          </div>

          <div className="panel overflow-hidden">
            {providers.map((p) => {
              const open = openProvider === p.name;
              return (
                <div key={p.name} className="border-b border-border-subtle last:border-b-0">
                  <button
                    onClick={() => setOpenProvider(open ? null : p.name)}
                    className="flex w-full items-center gap-3 px-4 py-2.5 text-left transition-colors hover:bg-surface-2/60"
                  >
                    <ChevronDown
                      size={13}
                      className={`text-faint transition-transform duration-150 ${open ? "" : "-rotate-90"}`}
                    />
                    <span className="w-40 truncate font-mono text-[13px] font-semibold text-ink">
                      {p.name}
                    </span>
                    <span className="flex-1" />
                    {p.has_key ? (
                      <span className="rounded bg-success/15 px-1.5 py-0.5 text-[10px] font-bold uppercase text-success">
                        key set
                      </span>
                    ) : p.needs_key ? (
                      <span className="rounded bg-warn/15 px-1.5 py-0.5 text-[10px] font-bold uppercase text-warn">
                        needs key
                      </span>
                    ) : (
                      <span className="rounded bg-surface-2 px-1.5 py-0.5 text-[10px] font-bold uppercase text-dim">
                        local
                      </span>
                    )}
                    <ChevronDown size={13} className="text-faint" />
                  </button>
                  {open && (
                    <form
                      onSubmit={(e) => onConnect(e, p.name, p.needs_key)}
                      className="flex items-center gap-2 border-t border-border-subtle bg-obsidian/30 px-4 py-2.5"
                    >
                      {p.needs_key ? (
                        <div className="relative flex-1">
                          <input
                            type={showKeys[p.name] ? "text" : "password"}
                            value={keys[p.name] ?? ""}
                            onChange={(e) => setKeys((s) => ({ ...s, [p.name]: e.target.value }))}
                            placeholder={`API key for ${p.name}…`}
                            className="input py-1.5 pr-16 font-mono text-[12px]"
                            autoComplete="off"
                            spellCheck={false}
                          />
                          <button
                            type="button"
                            onClick={() =>
                              setShowKeys((s) => ({ ...s, [p.name]: !s[p.name] }))
                            }
                            className="absolute right-2 top-1/2 -translate-y-1/2 text-[10.5px] font-semibold text-copper"
                          >
                            {showKeys[p.name] ? "hide" : "show"}
                          </button>
                        </div>
                      ) : (
                        <div className="flex-1 text-[12px] text-dim">
                          No key required — connects to a local endpoint.
                        </div>
                      )}
                      <button
                        type="submit"
                        disabled={busy[p.name]}
                        className="btn-copper shrink-0 px-3 py-1.5"
                      >
                        {busy[p.name] ? (
                          <Loader2 size={13} className="animate-spin" />
                        ) : (
                          <Check size={13} />
                        )}
                        Connect
                      </button>
                    </form>
                  )}
                </div>
              );
            })}
            {!providers.length && (
              <div className="px-4 py-6 text-center text-[12px] text-faint">
                No providers — the daemon has not sent a provider list yet.
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
