import { FormEvent, KeyboardEvent, useCallback, useEffect, useMemo, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import { AlertTriangle, CornerDownLeft, SendHorizonal, User } from "lucide-react";

import { ChatMessage } from "../lib/protocol";
import { useStore } from "../store";
import { EditDiffCardView, ToolCallCardView } from "./cards";

// ---------------------------------------------------------------------------
// Slash-command menu entries
// ---------------------------------------------------------------------------

const SLASH_COMMANDS = [
  { cmd: "/help", desc: "Show available commands" },
  { cmd: "/tools", desc: "List enabled tools" },
  { cmd: "/version", desc: "Show DIAD AGENT version" },
  { cmd: "/connect", desc: "Connect a provider (set API key)" },
  { cmd: "/models", desc: "List models for the current provider" },
  { cmd: "/status", desc: "Show connection and system status" },
];

// ---------------------------------------------------------------------------
// Single chat message
// ---------------------------------------------------------------------------

function Message({ msg }: { msg: ChatMessage }) {
  const isUser = msg.role === "user";
  const isSys = msg.role === "system";

  if (isSys) {
    return (
      <div
        className={`flex animate-fade-in gap-2 px-1 py-1 ${
          msg.error ? "text-danger" : "text-dim"
        }`}
      >
        <AlertTriangle size={13} className="mt-0.5 shrink-0" />
        <span className="text-[12.5px] leading-relaxed">{msg.content}</span>
      </div>
    );
  }

  if (isUser) {
    return (
      <div className="flex animate-fade-in justify-end px-1 py-1">
        <div className="max-w-[82%] rounded-panel rounded-br-sm bg-copper/15 px-3.5 py-2 text-[13.5px] leading-relaxed text-ink">
          <span className="whitespace-pre-wrap">{msg.content}</span>
        </div>
      </div>
    );
  }

  // assistant
  return (
    <div className="flex animate-fade-in gap-2.5 px-1 py-1.5">
      <span className="mt-1 flex h-6 w-6 shrink-0 items-center justify-center rounded-md bg-copper font-mono text-[11px] font-bold text-[#1a0f07]">
        D
      </span>
      <div className="min-w-0 flex-1">
        {msg.content && (
          <div className="md-body text-[13.5px] text-ink/95">
            <ReactMarkdown
              remarkPlugins={[remarkGfm]}
              rehypePlugins={[rehypeHighlight]}
            >
              {msg.content}
            </ReactMarkdown>
          </div>
        )}
        {!msg.done && msg.content === "" && (
          <div className="flex items-center gap-1.5 py-1 text-dim">
            <span className="h-1.5 w-1.5 animate-pulse-dot rounded-full bg-copper" />
            <span className="text-[12px]">working…</span>
          </div>
        )}
        {msg.toolCalls.map((c) => (
          <ToolCallCardView key={c.callId} card={c} />
        ))}
        {msg.diffs.map((d) => (
          <EditDiffCardView key={d.id} card={d} />
        ))}
        {!msg.done && <span className="cursor-blink ml-0.5 text-copper">▍</span>}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Chat panel
// ---------------------------------------------------------------------------

export default function ChatPanel() {
  const messages = useStore((s) => s.messages);
  const input = useStore((s) => s.input);
  const setInput = useStore((s) => s.setInput);
  const sendChat = useStore((s) => s.sendChat);
  const streaming = useStore((s) => s.streaming);
  const connected = useStore((s) => s.connected);
  const ws = useStore((s) => s.ws);

  const scrollRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [menuIdx, setMenuIdx] = useState(0);

  const filtered = useMemo(
    () =>
      input.startsWith("/")
        ? SLASH_COMMANDS.filter((c) => c.cmd.startsWith(input.split(" ")[0]))
        : [],
    [input]
  );

  const autoGrow = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 160) + "px";
  }, []);

  useEffect(autoGrow, [input, autoGrow]);

  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages]);

  useEffect(() => {
    setMenuIdx(0);
    setMenuOpen(input.startsWith("/") && filtered.length > 0);
  }, [input, filtered]);

  const submit = useCallback(
    (text: string) => {
      if (!text.trim()) return;
      if (text.trim() === "/connect") {
        useStore.getState().setProviderPickerOpen(true);
        return;
      }
      sendChat(text);
    },
    [sendChat]
  );

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (menuOpen && filtered.length > 0) {
        submit(filtered[menuIdx].cmd);
      } else {
        submit(input);
      }
    } else if (e.key === "ArrowDown" && menuOpen) {
      e.preventDefault();
      setMenuIdx((i) => (i + 1) % filtered.length);
    } else if (e.key === "ArrowUp" && menuOpen) {
      e.preventDefault();
      setMenuIdx((i) => (i - 1 + filtered.length) % filtered.length);
    } else if (e.key === "Escape" && menuOpen) {
      setMenuOpen(false);
    }
  };

  const onSubmit = (e: FormEvent) => {
    e.preventDefault();
    submit(input);
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* Messages */}
      <div ref={scrollRef} className="min-h-0 flex-1 overflow-y-auto px-4 py-3">
        <div className="mx-auto max-w-3xl space-y-2">
          {messages.map((m) => (
            <Message key={m.id} msg={m} />
          ))}
          {!messages.length && (
            <div className="py-16 text-center text-dim">
              Ask DIAD AGENT anything. <User size={14} className="inline" />
            </div>
          )}
        </div>
      </div>

      {/* Composer */}
      <div className="shrink-0 border-t border-border-subtle bg-surface/40 px-4 py-3">
        <div className="mx-auto max-w-3xl">
          <form onSubmit={onSubmit} className="glass relative rounded-panel">
            {/* slash command menu */}
            {menuOpen && (
              <div className="absolute bottom-full left-2 mb-2 w-72 overflow-hidden rounded-control border border-border-subtle bg-surface shadow-xl">
                {filtered.map((c, i) => (
                  <button
                    key={c.cmd}
                    type="button"
                    onClick={() => {
                      submit(c.cmd);
                      setMenuOpen(false);
                    }}
                    className={`flex w-full items-center gap-2 px-3 py-1.5 text-left transition-colors ${
                      i === menuIdx ? "bg-copper/15 text-ink" : "text-dim"
                    }`}
                  >
                    <span className="font-mono text-[12px] text-copper">{c.cmd}</span>
                    <span className="text-[11.5px]">{c.desc}</span>
                  </button>
                ))}
              </div>
            )}

            <textarea
              ref={textareaRef}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={onKeyDown}
              rows={1}
              placeholder={
                ws !== "connected"
                  ? "Connecting to DIAD daemon…"
                  : connected
                    ? "Message DIAD AGENT… (Enter to send, Shift+Enter for newline)"
                    : "No provider key configured — press F6 or type /connect"
              }
              className="block max-h-40 w-full resize-none bg-transparent px-4 pb-2 pt-3 pr-12 text-[13.5px] text-ink placeholder:text-faint outline-none"
            />

            <button
              type="submit"
              disabled={!input.trim() || streaming}
              className="absolute bottom-2 right-2 flex h-7 w-7 items-center justify-center rounded-control bg-copper text-[#1a0f07] transition-colors hover:bg-copper-hover disabled:cursor-not-allowed disabled:opacity-35"
              title="Send"
            >
              <SendHorizonal size={14} />
            </button>
          </form>
          <div className="mt-1.5 flex items-center justify-between px-1 text-[10.5px] text-faint">
            <span>
              <kbd className="rounded border border-border-subtle bg-surface-2 px-1 font-mono">Enter</kbd>{" "}
              send ·{" "}
              <kbd className="rounded border border-border-subtle bg-surface-2 px-1 font-mono">
                Shift+Enter
              </kbd>{" "}
              newline
            </span>
            <span className="flex items-center gap-1">
              <CornerDownLeft size={10} />
              {messages.length} messages
            </span>
          </div>
        </div>
      </div>
    </div>
  );
}
