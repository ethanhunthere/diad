import { Bot, ChevronRight, TerminalSquare, Sparkles } from "lucide-react";
import { useStore, sendMsg } from "../stores/useStore";
import { nextId } from "../lib/format";
import type { ProviderEntryDto } from "../lib/types";

const AGENT_META: Record<string, { desc: string; icon: string }> = {
  claude: { desc: "Anthropic Claude Code — terminal coding agent", icon: "✦" },
  codex: { desc: "OpenAI Codex — cloud coding agent", icon: "▣" },
  aider: { desc: "Aider — pair-programming CLI", icon: "⚡" },
  opencode: { desc: "OpenCode — local AI coding agent", icon: "◉" },
  gemini: { desc: "Google Gemini CLI — code & workspace agent", icon: "✧" },
};

const PROVIDER_META: Record<string, { desc: string }> = {
  anthropic: { desc: "Claude Opus / Sonnet — complex reasoning" },
  gemini: { desc: "Google Gemini 2.5 Pro" },
  openai: { desc: "OpenAI GPT-4o / o-series" },
  kimi: { desc: "Kimi K2.5 — 256k context, tool calling" },
  deepseek: { desc: "DeepSeek — code & reasoning" },
  groq: { desc: "Groq — ultra-fast inference" },
  ollama: { desc: "Local models via Ollama" },
};

/**
 * Agents view — installed external AI coding CLIs + the universal
 * provider matrix with connect buttons.
 */
export function AgentsPanel() {
  const agents = useStore((s) => s.agents);
  const providers = useStore((s) => s.providers);
  const setPendingConnectProvider = useStore((s) => s.setPendingConnectProvider);
  const connected = useStore((s) => s.connected);
  const wsConnected = useStore((s) => s.wsConnected);
  const streaming = useStore((s) => s.streaming);

  const delegate = (agent: string) => {
    const task = prompt(`Delegate task to ${agent}:`)?.trim();
    if (!task) return;
    sendMsg({ type: "agent.delegate", id: nextId("ad"), agent, task });
  };

  const connect = (p: ProviderEntryDto) => {
    if (p.has_key) {
      sendMsg({ type: "provider.switch", name: p.name });
    } else {
      setPendingConnectProvider(p.name);
    }
  };

  return (
    <div className="h-full overflow-y-auto px-5 py-4">
      <h2 className="text-sm font-semibold text-ink">Agents &amp; Providers</h2>
      <p className="mt-0.5 text-[12px] text-dim">
        External AI coding CLIs detected on this machine, and the universal provider
        matrix.
      </p>

      {/* External agents */}
      <section className="mt-4">
        <div className="mb-2 flex items-center gap-2 text-[11px] uppercase tracking-wider text-faint">
          <Bot size={12} /> External coding agents
        </div>
        <div className="grid grid-cols-1 gap-2 md:grid-cols-2 xl:grid-cols-3">
          {Object.entries(AGENT_META).map(([name, meta]) => {
            const installed = agents.includes(name);
            return (
              <div
                key={name}
                className={`panel p-3 transition-all duration-150 ease-out ${
                  installed ? "border-copper/25" : "opacity-70"
                }`}
              >
                <div className="flex items-center gap-2">
                  <span className="text-lg text-copper-hover">{meta.icon}</span>
                  <span className="font-mono text-[13px] font-medium text-ink">{name}</span>
                  {installed ? (
                    <span className="chip !text-success">installed</span>
                  ) : (
                    <span className="chip">not found</span>
                  )}
                </div>
                <div className="mt-1.5 text-[11.5px] text-dim">{meta.desc}</div>
                <button
                  className="btn-primary mt-2.5 h-6 gap-1 !py-0 text-[11px]"
                  disabled={!installed || !wsConnected || streaming}
                  onClick={() => delegate(name)}
                  title={installed ? `Delegate a task to ${name}` : `${name} is not installed`}
                >
                  <Sparkles size={12} /> Delegate task…
                </button>
              </div>
            );
          })}
        </div>
      </section>

      {/* Provider matrix */}
      <section className="mt-6">
        <div className="mb-2 flex items-center gap-2 text-[11px] uppercase tracking-wider text-faint">
          <TerminalSquare size={12} /> Provider matrix
        </div>
        <div className="panel overflow-hidden">
          <table className="w-full text-left">
            <thead>
              <tr className="border-b border-border-subtle text-[10.5px] uppercase tracking-wider text-faint">
                <th className="px-3 py-2 font-medium">Provider</th>
                <th className="hidden px-3 py-2 font-medium md:table-cell">Notes</th>
                <th className="px-3 py-2 font-medium">Status</th>
                <th className="px-3 py-2 text-right font-medium">Action</th>
              </tr>
            </thead>
            <tbody>
              {providers.map((p) => {
                const isActive = connected && useStore.getState().provider === p.name;
                return (
                  <tr
                    key={p.name}
                    className={`border-b border-border-subtle/60 last:border-0 ${
                      isActive ? "bg-copper/5" : "hover:bg-surface-2/50"
                    }`}
                  >
                    <td className="px-3 py-2">
                      <span className="flex items-center gap-2 font-mono text-[12.5px] text-ink">
                        {isActive && <span className="h-1.5 w-1.5 rounded-full bg-success" />}
                        {p.name}
                      </span>
                    </td>
                    <td className="hidden px-3 py-2 text-[11.5px] text-dim md:table-cell">
                      {PROVIDER_META[p.name]?.desc ?? "Universal provider preset"}
                    </td>
                    <td className="px-3 py-2">
                      {isActive ? (
                        <span className="text-[11px] text-success">active</span>
                      ) : p.has_key ? (
                        <span className="text-[11px] text-dim">key stored</span>
                      ) : (
                        <span className="text-[11px] text-faint">no key</span>
                      )}
                    </td>
                    <td className="px-3 py-2 text-right">
                      <button
                        className="btn-ghost h-6 gap-1 !py-0 text-[11px]"
                        disabled={!wsConnected || isActive}
                        onClick={() => connect(p)}
                        title={p.has_key ? `Switch to ${p.name}` : `Connect ${p.name} with an API key`}
                      >
                        {p.has_key ? "Switch" : "Connect"}
                        <ChevronRight size={12} />
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {providers.length === 0 && (
            <div className="px-3 py-4 text-center text-[11.5px] text-faint">
              {wsConnected ? "No providers yet — is the daemon healthy?" : "Daemon offline"}
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
