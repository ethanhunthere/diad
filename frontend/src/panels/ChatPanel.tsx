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

function MessageBubble({ m }: { m: ChatMessage }) {
  if (m.role === "system") {
    return (
      <div className="flex items-start gap-2 px-1 py-0.5 text-[12px] text-dim animate-fade-in">
        <span className="mt-1 h-1 w-1 shrink-0 rounded-full bg-faint" />
        <span className="break-words">{m.content}</span>
      </div>
    );
  }

  const isUser = m.role === "user";
  return (
    <div
      className={`flex ${isUser ? "justify-end" : "justify-start"} animate-fade-in`}
    >
      <div
        className={`max-w-[86%] rounded-panel border px-3 py-2 ${
          isUser
            ? "border-copper/30 bg-copper/15 text-ink"
            : "border-border-subtle bg-surface/70"
        }`}
      >
        {!isUser && (
          <div className="mb-1 flex items-center gap-1.5 text-[10px] uppercase tracking-wider text-faint">
            <span className="text-copper-hover">◈</span> assistant
            {m.streaming && (
              <span className="ml-1 inline-flex items-center gap-1 text-[10px] text-copper-hover">
                <span className="streaming-dot h-1.5 w-1.5 rounded-full bg-copper" />
                thinking…
              </span>
            )}
          </div>
        )}

        <div className={isUser ? "text-[13.5px] leading-relaxed" : "markdown"}>
          {isUser ? (
            <span className="whitespace-pre-wrap break-words">{m.content}</span>
          ) : (
            <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
              {m.content || (m.streaming ? "…" : "")}
            </ReactMarkdown>
          )}
        </div>

        {/* Tool call cards */}
        {m.tools.length > 0 && (
          <div className="mt-2 space-y-1.5">
            {m.tools.map((t) => (
              <ToolCallCard key={t.requestId} msgId={m.id} card={t} />
            ))}
          </div>
        )}

        {/* Edit diff cards */}
        {m.diffs.length > 0 && (
          <div className="mt-2 space-y-1.5">
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
 * Chat panel — streaming markdown conversation with tool call and
 * AI-edit diff cards, plus the auto-grow input.
 */
export function ChatPanel() {
  const messages = useStore((s) => s.messages);
  const streaming = useStore((s) => s.streaming);
  const wsConnected = useStore((s) => s.wsConnected);
  const scrollRef = useRef<HTMLDivElement>(null);
  const [stick, setStick] = useState(true);

  // Auto-scroll when new content arrives and the user is at the bottom.
  useEffect(() => {
    const el = scrollRef.current;
    if (el && stick) el.scrollTop = el.scrollHeight;
  }, [messages, stick]);

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el) return;
    setStick(el.scrollHeight - el.scrollTop - el.clientHeight < 60);
  };

  const send = (text: string) => {
    const trimmed = text.trim();
    if (!trimmed) return;
    useStore.getState().appendMessage({ role: "user", content: trimmed });
    sendMsg({ type: "chat.send", id: nextId("chat"), text: trimmed });
  };

  return (
    <div className="flex h-full flex-col">
      <div
        ref={scrollRef}
        onScroll={onScroll}
        className="min-h-0 flex-1 space-y-2 overflow-y-auto px-4 py-3"
      >
        {messages.length === 0 && (
          <div className="flex h-full flex-col items-center justify-center gap-3 text-center animate-fade-in">
            <div className="flex h-12 w-12 items-center justify-center rounded-panel border border-copper/30 bg-copper/10 text-2xl text-copper-hover">
              ◈
            </div>
            <div>
              <div className="text-sm font-semibold text-ink">DIAD AGENT</div>
              <div className="mt-1 text-[12px] text-dim">
                Ask anything — I route to your providers, tools, and offline intent
                router.
              </div>
            </div>
            <div className="mt-2 flex flex-wrap items-center justify-center gap-2 text-[11px] text-faint">
              <span className="kbd">/help</span>
              <span className="kbd">/tools</span>
              <span className="kbd">/version</span>
              <span className="kbd">Ctrl K</span>
              <span className="kbd">F6 provider</span>
            </div>
          </div>
        )}
        {messages.map((m) => (
          <MessageBubble key={m.id} m={m} />
        ))}
        {streaming && messages.length > 0 && (
          <div className="px-1 text-[11px] text-faint">◈ streaming…</div>
        )}
      </div>
      <ChatInput onSend={send} disabled={!wsConnected} />
    </div>
  );
}
