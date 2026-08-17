import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { useStore, sendMsg } from "../stores/useStore";
import { nextId } from "../lib/format";
import type { ChatMessage } from "../lib/types";
import { ToolCallCard } from "../components/ToolCallCard";
import { EditDiffCard } from "../components/EditDiffCard";
import { ChatInput } from "../components/ChatInput";
import { DiadLogo } from "../components/DiadLogo";

function MessageBubble({ m }: { m: ChatMessage }) {
  const [copied, setCopied] = useState(false);

  const copyContent = () => {
    if (!m.content) return;
    navigator.clipboard.writeText(m.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  if (m.role === "system") {
    return (
      <div className="flex items-start gap-2.5 px-3 py-1.5 text-[13px] text-slate-400 animate-fade-in font-mono bg-surface-2/40 rounded-xl border border-border-subtle/50 my-1">
        <span className="mt-1 h-2 w-2 shrink-0 rounded-full bg-amber-400 shadow-[0_0_8px_rgba(245,158,11,0.6)]" />
        <span className="break-words leading-relaxed">{m.content}</span>
      </div>
    );
  }

  const isUser = m.role === "user";
  return (
    <div
      className={`flex ${isUser ? "justify-end" : "justify-start"} animate-fade-in my-2 group`}
    >
      <div
        className={`max-w-[92%] sm:max-w-[85%] rounded-2xl border px-5 py-4 shadow-xl transition-all ${
          isUser
            ? "border-amber-500/50 bg-gradient-to-br from-amber-500/20 to-amber-600/10 text-slate-50 shadow-amber-950/20"
            : "border-border-subtle/80 bg-surface/95 backdrop-blur-xl text-slate-100 shadow-black/40 hover:border-border-strong"
        }`}
      >
        <div className="mb-2 flex items-center justify-between gap-3 text-[11.5px] font-semibold tracking-wider">
          <div className="flex items-center gap-2">
            {isUser ? (
              <span className="px-2 py-0.5 rounded-md bg-amber-500/30 text-amber-300 font-bold border border-amber-500/40 uppercase">
                ◈ USER
              </span>
            ) : (
              <div className="flex items-center gap-2">
                <span className="px-2 py-0.5 rounded-md bg-sky-500/20 text-sky-300 font-bold border border-sky-500/40 uppercase flex items-center gap-1">
                  ❖ DIAD AI
                </span>
                {m.streaming && (
                  <span className="inline-flex items-center gap-1.5 text-[11px] text-amber-400 font-normal">
                    <span className="streaming-dot h-2 w-2 rounded-full bg-amber-400" />
                    generating…
                  </span>
                )}
              </div>
            )}
          </div>

          {!isUser && (
            <button
              onClick={copyContent}
              className="opacity-0 group-hover:opacity-100 transition-opacity text-[11px] text-slate-400 hover:text-amber-400 flex items-center gap-1 px-1.5 py-0.5 rounded bg-surface-2/60 border border-border-subtle"
              title="Copy response"
            >
              {copied ? "✓ Copied" : "Copy"}
            </button>
          )}
        </div>

        <div className={isUser ? "text-[15.5px] leading-relaxed font-normal whitespace-pre-wrap break-words text-slate-50" : "markdown text-[15px] leading-relaxed text-slate-100"}>
          {isUser ? (
            <span>{m.content}</span>
          ) : (
            <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
              {m.content || (m.streaming ? "…" : "")}
            </ReactMarkdown>
          )}
        </div>

        {/* Tool call cards */}
        {m.tools.length > 0 && (
          <div className="mt-3.5 space-y-2.5">
            {m.tools.map((t) => (
              <ToolCallCard key={t.requestId} msgId={m.id} card={t} />
            ))}
          </div>
        )}

        {/* Edit diff cards */}
        {m.diffs.length > 0 && (
          <div className="mt-3.5 space-y-2.5">
            {m.diffs.map((d) => (
              <EditDiffCard key={d.requestId} card={d} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

/**
 * Chat panel — clean, spacious, high-contrast AI coding & pair-programming workspace.
 */
export function ChatPanel() {
  const messages = useStore((s) => s.messages);
  const clearMessages = useStore((s) => s.clearMessages);
  const setView = useStore((s) => s.setView);
  const provider = useStore((s) => s.provider);
  const model = useStore((s) => s.model);
  const streaming = useStore((s) => s.streaming);
  const wsConnected = useStore((s) => s.wsConnected);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [stick, setStick] = useState(true);

  useEffect(() => {
    const el = scrollRef.current;
    if (el && stick) el.scrollTop = el.scrollHeight;
  }, [messages, stick]);

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    setStick(el.scrollHeight - el.scrollTop - el.clientHeight < 80);
  };

  const send = (text: string) => {
    const trimmed = text.trim();
    if (!trimmed) return;
    const lower = trimmed.toLowerCase();
    if (lower === "/tuicolor" || lower === "/theme" || lower === "/personalize" || lower.startsWith("/tuicolor") || lower.startsWith("/theme")) {
      setView("settings");
      useStore.getState().appendMessage({
        role: "system",
        content: "◈ Opening the Settings and Appearance panel.",
      });
      return;
    }
    useStore.getState().appendMessage({ role: "user", content: trimmed });
    sendMsg({ type: "chat_send", id: nextId("chat"), text: trimmed });
  };

  const STARTER_PROMPTS = [
    { label: "📡 Manage Wi-Fi & Network", prompt: "Show available Wi-Fi networks and connect to the internet", desc: "Hardware wireless scanning & config" },
    { label: "⚡ System Health & Tuning", prompt: "Run full hardware diagnostics: CPU, RAM, battery, thermal, and storage", desc: "Inspect and optimize OS performance" },
    { label: "🦀 Build & Code Applications", prompt: "Help me design and implement high-performance code in this project", desc: "Autonomous AI coding & refactoring" },
    { label: "📂 Explore & Organize Files", prompt: "List files in the workspace and show recent project changes", desc: "Workspace navigation & file management" },
  ];

  return (
    <div className="flex h-full flex-col bg-obsidian">
      {/* Top chat action bar if messages exist */}
      {messages.length > 0 && (
        <div className="flex items-center justify-between border-b border-border-subtle/80 bg-surface/70 backdrop-blur-md px-5 py-2.5 text-[12px]">
          <div className="flex items-center gap-2.5 text-slate-300">
            <span className="text-amber-400 font-bold">◈</span>
            <span className="font-semibold text-slate-100">{provider || "DIAD"} · {model || "Agent"}</span>
            <span className="text-slate-500">({messages.length} messages)</span>
          </div>
          <div className="flex items-center gap-2">
            <button
              className="flex items-center gap-1.5 rounded-lg border border-border-subtle bg-surface-2 px-3 py-1.5 text-[12px] font-medium text-slate-300 hover:border-amber-500/50 hover:text-amber-300 transition-colors shadow-sm"
              onClick={clearMessages}
              title="Start a fresh conversation"
            >
              + New Chat
            </button>
          </div>
        </div>
      )}

      <div
        ref={scrollRef}
        onScroll={onScroll}
        className="min-h-0 flex-1 space-y-4 overflow-y-auto px-6 py-5"
      >
        {messages.length === 0 && (
          <div className="flex h-full flex-col items-center justify-center gap-6 text-center max-w-xl mx-auto px-4 animate-fade-in my-auto">
            <DiadLogo size={56} showWordmark={false} />
            <div>
              <h1 className="text-2xl sm:text-3xl font-black tracking-tight text-slate-100 font-mono">
                DIAD 2040 AUTONOMOUS CORE
              </h1>
              <p className="mt-2 text-[14.5px] text-slate-400 max-w-md mx-auto leading-relaxed">
                Intent-driven operating system intelligence. Code, configure hardware, and automate your computer with AI.
              </p>
            </div>

            {/* Quick starter cards */}
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 w-full mt-2 text-left">
              {STARTER_PROMPTS.map((item, idx) => (
                <button
                  key={idx}
                  className="rounded-xl border border-border-subtle bg-surface-2/80 p-4 hover:border-amber-500/60 hover:bg-surface-3/90 hover:shadow-xl hover:shadow-amber-950/20 transition-all group"
                  onClick={() => {
                    send(item.prompt);
                  }}
                >
                  <div className="text-[14px] font-semibold text-slate-200 group-hover:text-amber-400 transition-colors">
                    {item.label}
                  </div>
                  <div className="text-[12px] text-slate-400 mt-1">
                    {item.desc}
                  </div>
                </button>
              ))}
            </div>

            <div className="mt-3 flex flex-wrap items-center justify-center gap-2 text-[12px] text-slate-500">
              <span className="kbd bg-surface-2 border border-border-subtle px-2 py-1 rounded text-slate-400">F5 TUI Mode</span>
              <span className="kbd bg-surface-2 border border-border-subtle px-2 py-1 rounded text-slate-400">Ctrl+K Commands</span>
              <span className="kbd bg-surface-2 border border-border-subtle px-2 py-1 rounded text-slate-400">F6 Model</span>
            </div>
          </div>
        )}
        {messages.map((m) => (
          <MessageBubble key={m.id} m={m} />
        ))}
        {streaming && messages.length > 0 && (
          <div className="px-3 py-1 text-[13px] text-amber-400 flex items-center gap-2 animate-pulse font-mono">
            <span className="streaming-dot h-2 w-2 rounded-full bg-amber-400" />
            <span>DIAD Agent is reasoning…</span>
          </div>
        )}
      </div>
      <ChatInput onSend={send} disabled={!wsConnected} />
    </div>
  );
}
