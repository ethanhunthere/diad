import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { Copy, Check, Terminal, Wrench, Shield, HelpCircle, Plus, ChevronRight, Eye } from "lucide-react";
import { useStore, sendMsg } from "../stores/useStore";
import { nextId } from "../lib/format";
import type { ChatMessage } from "../lib/types";
import { ToolCallCard } from "../components/ToolCallCard";
import { EditDiffCard } from "../components/EditDiffCard";
import { ChatInput } from "../components/ChatInput";
import { AgentExplainerModal } from "../components/AgentExplainerModal";
import { ThinkingOrb } from "thinking-orbs";

function CodeBlock({ children, className }: { children: React.ReactNode; className?: string }) {
  const [copied, setCopied] = useState(false);
  const match = /language-(\w+)/.exec(className || "");
  const lang = match ? match[1] : "";
  const codeText = String(children).replace(/\n$/, "");

  const copyCode = () => {
    navigator.clipboard.writeText(codeText);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="relative my-3.5 overflow-hidden rounded-xl border border-[#202623] bg-[#090A0A]">
      <div className="flex items-center justify-between border-b border-[#202623] bg-[#161918] px-3.5 py-1.5 text-[11.5px] font-mono text-[#8A968E]">
        <span className="font-semibold text-[#E5EAE6]">{lang || "code"}</span>
        <button
          onClick={copyCode}
          className="flex items-center gap-1.5 rounded px-2 py-0.5 text-[11px] text-[#8A968E] hover:bg-[#1E2321] hover:text-[#E5EAE6] transition-colors"
          title="Copy code"
        >
          {copied ? (
            <>
              <Check size={12} className="text-[#38A169]" />
              <span className="text-[#38A169]">Copied</span>
            </>
          ) : (
            <>
              <Copy size={12} />
              <span>Copy</span>
            </>
          )}
        </button>
      </div>
      <div className="overflow-x-auto p-4 text-[13.5px] font-mono leading-relaxed text-[#E5EAE6]">
        <code>{children}</code>
      </div>
    </div>
  );
}

function MessageBubble({ m }: { m: ChatMessage }) {
  const [copied, setCopied] = useState(false);
  const [previewImage, setPreviewImage] = useState<string | null>(null);

  const copyContent = () => {
    if (!m.content) return;
    navigator.clipboard.writeText(m.content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  if (m.role === "system") {
    return (
      <div className="flex items-center gap-2 px-4 py-2.5 text-[12px] text-dim animate-fade-in font-mono bg-surface-2/90 backdrop-blur-md rounded-xl border border-border-subtle my-2 max-w-2xl mx-auto shadow-sm">
        <span className="font-semibold text-copper">[SYSTEM]</span>
        <span className="break-words leading-relaxed text-ink">{m.content}</span>
      </div>
    );
  }

  const isUser = m.role === "user";
  return (
    <div className={`flex ${isUser ? "justify-end" : "justify-start"} animate-spring-in my-4 group`}>
      {previewImage && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/90 p-4 backdrop-blur-lg"
          onClick={() => setPreviewImage(null)}
        >
          <img
            src={previewImage}
            alt="Preview"
            className="max-h-[90vh] max-w-[90vw] rounded-2xl border border-border-subtle object-contain shadow-2xl"
          />
        </div>
      )}

      <div
        className={`max-w-[96%] sm:max-w-[88%] rounded-2xl transition-all ${
          isUser
            ? "border border-border-subtle bg-surface-2 px-5 py-3.5 text-ink shadow-md hover:border-copper/40"
            : "border border-transparent bg-transparent text-ink px-2 sm:px-4 py-2"
        }`}
      >
        {/* Message Header */}
        <div className="mb-2 flex items-center justify-between gap-3 text-[12px] font-semibold">
          <div className="flex items-center gap-2">
            {isUser ? (
              <span className="text-[12px] font-bold text-ink font-mono">
                You
              </span>
            ) : (
              <div className="flex items-center gap-2">
                <span className="text-[12.5px] font-bold text-ink font-mono">
                  DIAD Agent
                </span>
                {m.streaming && (
                  <span className="text-[11px] text-copper font-mono font-medium animate-pulse">
                    executing…
                  </span>
                )}
              </div>
            )}
          </div>

          {!isUser && m.content && (
            <button
              onClick={copyContent}
              className="opacity-0 group-hover:opacity-100 transition-all text-[11px] text-dim hover:text-ink flex items-center gap-1 px-2 py-0.5 rounded-md bg-surface-2 border border-border-subtle shadow-sm hover:border-copper"
              title="Copy message"
            >
              {copied ? (
                <>
                  <Check size={11} className="text-success" />
                  <span className="text-success font-medium">Copied</span>
                </>
              ) : (
                <>
                  <Copy size={11} />
                  <span>Copy</span>
                </>
              )}
            </button>
          )}
        </div>

        {/* Attached Images in User Message */}
        {m.images && m.images.length > 0 && (
          <div className="mb-3 flex flex-wrap gap-2.5">
            {m.images.map((imgUrl, idx) => (
              <div
                key={idx}
                className="group/img relative cursor-pointer overflow-hidden rounded-xl border border-border-subtle hover:border-copper transition-all"
                onClick={() => setPreviewImage(imgUrl)}
              >
                <img
                  src={imgUrl}
                  alt={`Attachment ${idx + 1}`}
                  className="h-24 w-24 object-cover sm:h-32 sm:w-32"
                />
                <div className="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 group-hover/img:opacity-100 transition-opacity">
                  <Eye size={18} className="text-white" />
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Thinking Orb animation while agent is answering */}
        {!isUser && m.streaming && (
          <div className="mb-4 flex items-center gap-4 p-3.5 rounded-2xl bg-surface-2 border border-border-subtle shadow-lg animate-fade-in w-fit">
            <div className="flex items-center gap-2">
              <ThinkingOrb state="breathing" size={64} speed={2.25} theme="dark" />
              <ThinkingOrb state="listening" size={64} theme="dark" />
            </div>
            <div className="flex flex-col font-mono text-[12px]">
              <span className="font-semibold text-ink">DIAD Agent thinking…</span>
              <span className="text-dim text-[11px]">Streaming live response</span>
            </div>
          </div>
        )}

        {/* Message Content */}
        <div className={isUser ? "text-[15px] sm:text-[15.5px] leading-relaxed whitespace-pre-wrap break-words text-ink" : "markdown text-[15px] sm:text-[15.5px] leading-[1.75] text-ink"}>
          {isUser ? (
            <span>{m.content}</span>
          ) : (
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              rehypePlugins={[rehypeHighlight]}
              components={{
                code({ node, inline, className, children, ...props }: any) {
                  if (inline) {
                    return (
                      <code className="rounded bg-surface-3 px-1.5 py-0.5 font-mono text-[13px] text-ink border border-border-subtle" {...props}>
                        {children}
                      </code>
                    );
                  }
                  return <CodeBlock className={className}>{children}</CodeBlock>;
                },
                table({ children }) {
                  return (
                    <div className="my-4 overflow-x-auto rounded-xl border border-border-subtle bg-surface">
                      <table className="w-full text-left text-[13.5px] border-collapse">
                        {children}
                      </table>
                    </div>
                  );
                },
                th({ children }) {
                  return (
                    <th className="border-b border-border-subtle bg-surface-2 px-4 py-2.5 font-semibold text-ink">
                      {children}
                    </th>
                  );
                },
                td({ children }) {
                  return (
                    <td className="border-b border-border-subtle/60 px-4 py-2.5 text-dim">
                      {children}
                    </td>
                  );
                },
                blockquote({ children }) {
                  return (
                    <blockquote className="my-3 border-l-2 border-copper bg-surface-2/70 px-4 py-2 text-dim rounded-r-lg">
                      {children}
                    </blockquote>
                  );
                },
              }}
            >
              {m.content || (m.streaming ? "…" : "")}
            </ReactMarkdown>
          )}
        </div>

        {/* Tool call cards */}
        {m.tools.length > 0 && (
          <div className="mt-4 space-y-3">
            {m.tools.map((t) => (
              <ToolCallCard key={t.requestId} msgId={m.id} card={t} />
            ))}
          </div>
        )}

        {/* Edit diff cards */}
        {m.diffs.length > 0 && (
          <div className="mt-4 space-y-3">
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
 * Chat panel — clean, spacious, world-class AI coding & pair-programming workspace.
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
  const [showExplainer, setShowExplainer] = useState(false);

  useEffect(() => {
    const el = scrollRef.current;
    if (el && stick) el.scrollTop = el.scrollHeight;
  }, [messages, stick]);

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    setStick(el.scrollHeight - el.scrollTop - el.clientHeight < 80);
  };

  const send = (text: string, images?: string[]) => {
    const trimmed = text.trim();
    if (!trimmed && (!images || images.length === 0)) return;
    const lower = trimmed.toLowerCase();
    if (lower === "/tuicolor" || lower === "/theme" || lower === "/personalize" || lower.startsWith("/tuicolor") || lower.startsWith("/theme")) {
      setView("settings");
      useStore.getState().appendMessage({
        role: "system",
        content: "Opening Settings & Appearance panel.",
      });
      return;
    }
    if (lower === "/help" || lower === "/explainer" || lower === "/guide") {
      setShowExplainer(true);
      return;
    }

    useStore.getState().appendMessage({
      role: "user",
      content: trimmed,
      images,
    });

    sendMsg({
      type: "chat_send",
      id: nextId("chat"),
      text: trimmed,
      images: images && images.length > 0 ? images : undefined,
    });
  };

  const QUICK_PROMPTS = [
    { label: "Check Wi-Fi & Network", cmd: "/wifi", icon: Wrench },
    { label: "Hardware Diagnostics", cmd: "/status", icon: Shield },
    { label: "How DIAD Agent Works", action: () => setShowExplainer(true), icon: HelpCircle },
    { label: "Open TUI Client", cmd: "/tui", icon: Terminal },
  ];

  return (
    <div className="flex h-full flex-col bg-obsidian">
      {showExplainer && <AgentExplainerModal onClose={() => setShowExplainer(false)} />}

      {/* Top chat action bar */}
      <div className="flex h-11 items-center justify-between border-b border-border-subtle bg-surface px-4 sm:px-6 text-[12.5px] select-none">
        <div className="flex items-center gap-2 text-dim">
          <span className="font-semibold text-ink">{provider || "DIAD"} · {model || "Agent"}</span>
          {messages.length > 0 && (
            <span className="text-faint text-[11.5px]">({messages.length} messages)</span>
          )}
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => setShowExplainer(true)}
            className="flex items-center gap-1.5 rounded-lg border border-border-subtle bg-surface-2 px-2.5 py-1 text-[11.5px] font-medium text-dim hover:border-copper hover:text-ink transition-colors"
            title="How DIAD Agent Works"
          >
            <HelpCircle size={13} className="text-copper" />
            <span className="hidden sm:inline">How it Works</span>
          </button>
          {messages.length > 0 && (
            <button
              className="flex items-center gap-1.5 rounded-lg border border-border-subtle bg-surface-2 px-2.5 py-1 text-[11.5px] font-medium text-dim hover:border-copper hover:text-ink transition-colors"
              onClick={clearMessages}
              title="Start a fresh conversation"
            >
              <Plus size={13} className="text-copper" />
              <span>New Chat</span>
            </button>
          )}
        </div>
      </div>

      <div
        ref={scrollRef}
        onScroll={onScroll}
        className="min-h-0 flex-1 space-y-4 overflow-y-auto px-4 sm:px-8 py-6"
      >
        {messages.length === 0 && (
          <div className="flex h-full flex-col items-center justify-center text-center px-4 my-auto animate-fade-in max-w-xl mx-auto">
            <div className="flex h-12 w-12 items-center justify-center rounded-2xl border border-border-subtle bg-surface-2 text-copper shadow-xl mb-4 font-mono font-bold text-base ring-1 ring-copper/30">
              DIAD
            </div>
            <h3 className="text-[18px] font-bold text-ink">
              DIAD Studio
            </h3>
            <p className="mt-1.5 text-[13.5px] text-dim max-w-md leading-relaxed">
              Intent-driven OS execution engine. Run shell tools, manage filesystem diffs, paste images, or control native desktop subsystems.
            </p>

            {/* Quick action cards */}
            <div className="mt-6 grid grid-cols-1 sm:grid-cols-2 gap-2.5 w-full">
              {QUICK_PROMPTS.map((p, idx) => {
                const Icon = p.icon;
                return (
                  <button
                    key={idx}
                    onClick={() => {
                      if (p.action) p.action();
                      else if (p.cmd) send(p.cmd);
                    }}
                    className="flex items-center justify-between rounded-xl border border-border-subtle bg-surface-2 p-3 text-left hover:border-copper hover:bg-surface-3 hover-elevate transition-all group shadow-sm"
                  >
                    <div className="flex items-center gap-2.5">
                      <Icon size={16} className="text-copper" />
                      <span className="text-[13px] font-semibold text-ink">
                        {p.label}
                      </span>
                    </div>
                    <ChevronRight size={14} className="text-faint group-hover:text-ink transition-colors" />
                  </button>
                );
              })}
            </div>
          </div>
        )}

        {messages.map((m) => (
          <MessageBubble key={m.id} m={m} />
        ))}

        {streaming && messages.length > 0 && !messages[messages.length - 1]?.streaming && (
          <div className="flex items-center gap-4 px-4 py-3 rounded-2xl bg-surface-2 border border-border-subtle shadow-lg animate-fade-in w-fit my-2">
            <div className="flex items-center gap-2">
              <ThinkingOrb state="breathing" size={64} speed={2.25} theme="dark" />
              <ThinkingOrb state="listening" size={64} theme="dark" />
            </div>
            <div className="flex flex-col font-mono text-[12px]">
              <span className="font-semibold text-ink">Thinking & preparing answer…</span>
              <span className="text-dim text-[11px]">Connecting to model stream</span>
            </div>
          </div>
        )}
      </div>

      <ChatInput onSend={send} disabled={!wsConnected} />
    </div>
  );
}
